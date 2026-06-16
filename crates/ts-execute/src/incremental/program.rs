use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_outputpaths as outputpaths;
use ts_tracing as tracing;
use ts_tspath as tspath;

use super::{
    DiagnosticsOrBuildInfoDiagnosticsWithFileName, Host, Snapshot, collect_all_affected_files,
    emit_files, programtosnapshot::program_to_snapshot, snapshot_to_build_info,
};

pub type SignatureUpdateKind = u8;

pub const SIGNATURE_UPDATE_KIND_COMPUTED_DTS: SignatureUpdateKind = 0;
pub const SIGNATURE_UPDATE_KIND_STORED_AT_EMIT: SignatureUpdateKind = 1;
pub const SIGNATURE_UPDATE_KIND_USED_VERSION: SignatureUpdateKind = 2;

pub struct Program {
    pub snapshot: Snapshot,
    pub program: Option<compiler::Program>,
    pub host: Option<Box<dyn Host>>,

    // Testing data
    pub testing_data: Option<TestingData>,
}

pub(crate) trait MaybeProgramExt {
    fn as_program(&self) -> &compiler::Program;
    fn as_program_mut(&mut self) -> &mut compiler::Program;
    fn is_source_file_default_library(&self, path: tspath::Path) -> bool;
    fn skip_type_checking(&self, source_file: &ast::SourceFile, ignore_no_check: bool) -> bool;
    fn emit(
        &mut self,
        ctx: core::Context,
        options: compiler::EmitOptions,
    ) -> Option<compiler::EmitResult>;
    fn get_source_file_by_path(&self, path: &tspath::Path) -> Option<ast::SourceFile>;
    fn single_threaded(&self) -> bool;
    fn source_file_may_be_emitted(
        &self,
        source_file: &ast::SourceFile,
        force_dts_emit: bool,
    ) -> bool;
    fn get_declaration_diagnostics(
        &mut self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>;
    fn host(&self) -> &dyn compiler::CompilerHost;
}

impl MaybeProgramExt for Option<compiler::Program> {
    fn as_program(&self) -> &compiler::Program {
        self.as_ref()
            .expect("incremental Program requires compiler Program")
    }

    fn as_program_mut(&mut self) -> &mut compiler::Program {
        self.as_mut()
            .expect("incremental Program requires compiler Program")
    }

    fn is_source_file_default_library(&self, path: tspath::Path) -> bool {
        self.as_program().is_source_file_default_library(path)
    }

    fn skip_type_checking(&self, source_file: &ast::SourceFile, ignore_no_check: bool) -> bool {
        self.as_program()
            .skip_type_checking(source_file, ignore_no_check)
    }

    fn emit(
        &mut self,
        ctx: core::Context,
        options: compiler::EmitOptions,
    ) -> Option<compiler::EmitResult> {
        compiler::ProgramLike::emit(self.as_program_mut(), ctx, options)
    }

    fn get_source_file_by_path(&self, path: &tspath::Path) -> Option<ast::SourceFile> {
        self.as_program().get_source_file_by_path(path.clone())
    }

    fn single_threaded(&self) -> bool {
        self.as_program().single_threaded()
    }

    fn source_file_may_be_emitted(
        &self,
        source_file: &ast::SourceFile,
        force_dts_emit: bool,
    ) -> bool {
        self.as_program()
            .source_file_may_be_emitted(source_file, force_dts_emit)
    }

    fn get_declaration_diagnostics(
        &mut self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        compiler::ProgramLike::get_declaration_diagnostics(
            self.as_program_mut(),
            ctx,
            file.as_ref(),
        )
    }

    fn host(&self) -> &dyn compiler::CompilerHost {
        self.as_program().host()
    }
}

pub fn new_program(
    program: compiler::Program,
    old_program: Option<&Program>,
    host: Box<dyn Host>,
    testing: bool,
) -> Program {
    let mut incremental_program = Program {
        snapshot: program_to_snapshot(&program, old_program, testing),
        program: Some(program),
        host: Some(host),
        testing_data: None,
    };

    if testing {
        let old_program_semantic_diagnostics_per_file = if let Some(old_program) = old_program {
            old_program.snapshot.semantic_diagnostics_per_file.clone()
        } else {
            HashMap::new()
        };
        incremental_program.testing_data = Some(TestingData {
            semantic_diagnostics_per_file: incremental_program
                .snapshot
                .semantic_diagnostics_per_file
                .clone(),
            old_program_semantic_diagnostics_per_file,
            refreshed_semantic_diagnostics: HashSet::new(),
            updated_signature_kinds: HashMap::new(),
        });
    }
    incremental_program
}

#[derive(Clone)]
pub struct TestingData {
    pub semantic_diagnostics_per_file:
        HashMap<tspath::Path, DiagnosticsOrBuildInfoDiagnosticsWithFileName>,
    pub old_program_semantic_diagnostics_per_file:
        HashMap<tspath::Path, DiagnosticsOrBuildInfoDiagnosticsWithFileName>,
    pub refreshed_semantic_diagnostics: HashSet<tspath::Path>,
    pub updated_signature_kinds: HashMap<tspath::Path, SignatureUpdateKind>,
}

impl Program {
    pub fn get_testing_data(&self) -> Option<&TestingData> {
        self.testing_data.as_ref()
    }

    fn sync_testing_semantic_diagnostics_per_file(&mut self) {
        if let Some(testing_data) = self.testing_data.as_mut() {
            testing_data.semantic_diagnostics_per_file =
                self.snapshot.semantic_diagnostics_per_file.clone();
        }
    }

    pub fn panic_if_no_program(&self, method: &str) {
        if self.program.is_none() {
            panic!("{method}: should not be called without program");
        }
    }

    pub fn get_program(&self) -> &compiler::Program {
        self.panic_if_no_program("GetProgram");
        self.program.as_ref().unwrap()
    }

    pub fn has_changed_dts_file(&self) -> bool {
        self.snapshot.has_changed_dts_file
    }

    // Options implements compiler.AnyProgram interface.
    pub fn options(&self) -> core::CompilerOptions {
        self.snapshot.options.clone()
    }

    // CommonSourceDirectory implements compiler.AnyProgram interface.
    pub fn common_source_directory(&self) -> String {
        self.panic_if_no_program("CommonSourceDirectory");
        self.get_program().common_source_directory()
    }

    // Program implements compiler.AnyProgram interface.
    pub fn program(&self) -> &compiler::Program {
        self.panic_if_no_program("Program");
        self.get_program()
    }

    // IsSourceFileDefaultLibrary implements compiler.AnyProgram interface.
    pub fn is_source_file_default_library(&self, path: tspath::Path) -> bool {
        self.panic_if_no_program("IsSourceFileDefaultLibrary");
        self.get_program().is_source_file_default_library(path)
    }

    // GetSourceFiles implements compiler.AnyProgram interface.
    pub fn get_source_files(&self) -> Vec<ast::SourceFile> {
        self.panic_if_no_program("GetSourceFiles");
        self.get_program().get_source_files()
    }

    // GetSourceFile implements compiler.AnyProgram interface.
    pub fn get_source_file(&self, path: &str) -> Option<ast::SourceFile> {
        self.panic_if_no_program("GetSourceFile");
        self.get_program().get_source_file(path)
    }

    // GetConfigFileParsingDiagnostics implements compiler.AnyProgram interface.
    pub fn get_config_file_parsing_diagnostics(&self) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetConfigFileParsingDiagnostics");
        self.get_program().get_config_file_parsing_diagnostics()
    }

    // GetSyntacticDiagnostics implements compiler.AnyProgram interface.
    pub fn get_syntactic_diagnostics(
        &self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetSyntacticDiagnostics");
        self.get_program()
            .get_syntactic_diagnostics(ctx, file.as_ref())
    }

    // GetBindDiagnostics implements compiler.AnyProgram interface.
    pub fn get_bind_diagnostics(
        &self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetBindDiagnostics");
        self.get_program().get_bind_diagnostics(ctx, file.as_ref())
    }

    pub fn get_program_diagnostics(&self) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetProgramDiagnostics");
        self.get_program().get_program_diagnostics()
    }

    pub fn get_global_diagnostics(&self, ctx: core::Context) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetGlobalDiagnostics");
        self.get_program().get_global_diagnostics(ctx)
    }

    // GetSemanticDiagnostics implements compiler.AnyProgram interface.
    pub fn get_semantic_diagnostics(
        &mut self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetSemanticDiagnostics");
        if self.snapshot.options.no_check.is_true() {
            return Vec::new();
        }

        // Ensure all the diagnsotics are cached
        self.collect_semantic_diagnostics_of_affected_files(
            ctx.clone(),
            file.as_ref().map(ast::SourceFile::share_readonly),
        );
        if ctx.err().is_some() {
            return Vec::new();
        }

        // Return result from cache
        if let Some(file) = file {
            return self.get_semantic_diagnostics_of_file(&file);
        }

        let mut diagnostics = Vec::new();
        for file in self.get_program().get_source_files() {
            diagnostics.extend(self.get_semantic_diagnostics_of_file(&file));
        }
        diagnostics
    }

    pub fn get_semantic_diagnostics_of_file(&self, file: &ast::SourceFile) -> Vec<ast::Diagnostic> {
        let mut cached_diagnostics = self
            .snapshot
            .semantic_diagnostics_per_file
            .get(&file.path())
            .cloned()
            .unwrap_or_else(|| {
                panic!("After handling all the affected files, there shouldnt be more changes")
            });
        let mut result = compiler::filter_no_emit_semantic_diagnostics(
            cached_diagnostics.get_diagnostics(self.get_program(), file),
            &self.snapshot.options,
        );
        result.extend(self.get_program().get_include_processor_diagnostics(file));
        result
    }

    // GetDeclarationDiagnostics implements compiler.AnyProgram interface.
    pub fn get_declaration_diagnostics(
        &mut self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetDeclarationDiagnostics");
        let result = emit_files(
            ctx,
            self,
            compiler::EmitOptions {
                target_source_file: file,
                ..Default::default()
            },
            true,
        );
        if let Some(result) = result {
            return result.diagnostics;
        }
        Vec::new()
    }

    // GetSuggestionDiagnostics implements compiler.AnyProgram interface.
    pub fn get_suggestion_diagnostics(
        &self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.panic_if_no_program("GetSuggestionDiagnostics");
        self.get_program()
            .get_suggestion_diagnostics(ctx, file.as_ref())
    }

    // GetModeForUsageLocation implements compiler.AnyProgram interface.
    pub fn emit(
        &mut self,
        ctx: core::Context,
        options: compiler::EmitOptions,
    ) -> Option<compiler::EmitResult> {
        self.panic_if_no_program("Emit");

        let mut result = None;
        if self.snapshot.options.no_emit.is_true() {
            result = Some(compiler::EmitResult {
                emit_skipped: true,
                ..Default::default()
            });
        } else {
            result = compiler::handle_no_emit_on_error(
                ctx.clone(),
                self,
                options.target_source_file.as_ref(),
            );
            if ctx.err().is_some() {
                return None;
            }
        }
        if let Some(mut result) = result {
            if options.target_source_file.is_some() {
                return Some(result);
            }

            // Emit buildInfo and combine result
            let build_info_result = self.emit_build_info(ctx, options);
            if let Some(build_info_result) = build_info_result {
                result.diagnostics.extend(build_info_result.diagnostics);
                result.emitted_files.extend(build_info_result.emitted_files);
            }
            return Some(result);
        }
        emit_files(ctx, self, options, false)
    }

    // Handle affected files and cache the semantic diagnostics for all of them or the file asked for
    pub fn collect_semantic_diagnostics_of_affected_files(
        &mut self,
        ctx: core::Context,
        file: Option<ast::SourceFile>,
    ) {
        if self.snapshot.can_use_incremental_state() {
            // Get all affected files
            let mut affected_program = Program {
                snapshot: self.snapshot.clone(),
                program: self.program.take(),
                host: None,
                testing_data: self.testing_data.take(),
            };
            collect_all_affected_files(ctx.clone(), &mut affected_program);
            self.program = affected_program.program;
            self.snapshot = affected_program.snapshot;
            self.testing_data = affected_program.testing_data;
            if ctx.err().is_some() {
                return;
            }

            self.sync_testing_semantic_diagnostics_per_file();

            if self.snapshot.semantic_diagnostics_per_file.len()
                == self.get_program().get_source_files().len()
            {
                // If we have all the files,
                return;
            }
        }

        let mut affected_files = Vec::new();
        if let Some(file) = file {
            if self
                .snapshot
                .semantic_diagnostics_per_file
                .contains_key(&file.path())
            {
                return;
            }
            affected_files.push(file);
        } else {
            for file in self.get_program().get_source_files() {
                if !self
                    .snapshot
                    .semantic_diagnostics_per_file
                    .contains_key(&file.path())
                {
                    affected_files.push(file);
                }
            }
        }

        // Get their diagnostics and cache them
        let diagnostics_per_file = self
            .get_program()
            .get_semantic_diagnostics_without_no_emit_filtering(ctx.clone(), &affected_files);
        // commit changes if no err
        if ctx.err().is_some() {
            return;
        }

        // Commit changes to snapshot
        for (file, diagnostics) in diagnostics_per_file {
            if let Some(testing_data) = self.testing_data.as_mut() {
                testing_data
                    .refreshed_semantic_diagnostics
                    .insert(file.path());
            }
            self.snapshot.semantic_diagnostics_per_file.insert(
                file.path(),
                DiagnosticsOrBuildInfoDiagnosticsWithFileName {
                    diagnostics,
                    ..Default::default()
                },
            );
        }
        self.sync_testing_semantic_diagnostics_per_file();
        if self.snapshot.semantic_diagnostics_per_file.len()
            == self.get_program().get_source_files().len()
            && self.snapshot.check_pending
            && !self.snapshot.options.no_check.is_true()
        {
            self.snapshot.check_pending = false;
        }
        self.snapshot
            .build_info_emit_pending
            .store(true, Ordering::SeqCst);
    }

    pub fn emit_build_info(
        &mut self,
        ctx: core::Context,
        options: compiler::EmitOptions,
    ) -> Option<compiler::EmitResult> {
        let mut tracing = self.get_program().tracing();
        let _trace = tracing.as_mut().map(|tr| {
            tr.push(
                tracing::Phase::Emit,
                "emitBuildInfo",
                HashMap::<String, serde_json::Value>::new(),
                true,
            )
        });
        let build_info_file_name = outputpaths::get_build_info_file_name(
            &compiler::outputpaths_compiler_options(&self.snapshot.options),
            tspath::ComparePathsOptions {
                current_directory: self.get_program().get_current_directory(),
                use_case_sensitive_file_names: self.get_program().use_case_sensitive_file_names(),
            },
        );
        if build_info_file_name.is_empty()
            || self.get_program().is_emit_blocked(&build_info_file_name)
        {
            return None;
        }
        if self.snapshot.has_errors == core::TS_UNKNOWN {
            let program = self
                .program
                .take()
                .expect("EmitBuildInfo: should not be called without program");
            self.ensure_has_errors_for_state(ctx.clone(), &program);
            self.program = Some(program);
            if self.snapshot.has_errors != self.snapshot.has_errors_from_old_state
                || self.snapshot.has_semantic_errors
                    != self.snapshot.has_semantic_errors_from_old_state
            {
                self.snapshot
                    .build_info_emit_pending
                    .store(true, Ordering::SeqCst);
            }
        }
        if !self.snapshot.build_info_emit_pending.load(Ordering::SeqCst) {
            return None;
        }
        if ctx.err().is_some() {
            return None;
        }
        let build_info =
            snapshot_to_build_info(&self.snapshot, self.get_program(), &build_info_file_name);
        let text = serde_json::to_string(&build_info)
            .unwrap_or_else(|err| panic!("Failed to marshal build info: {err}"));
        let err = {
            let mut data = compiler::WriteFileData {
                build_info: Some(Box::new(build_info.clone())),
                ..Default::default()
            };
            if let Some(write_file) = options.write_file {
                (write_file.borrow_mut())(&build_info_file_name, &text, Some(&mut data))
            } else {
                self.get_program()
                    .host()
                    .fs()
                    .write_file(&build_info_file_name, &text)
                    .map_err(|err| err.to_string())
            }
        };
        if let Err(err) = err {
            let diagnostic_args: Vec<diagnostics::Argument> = vec![
                Box::new(build_info_file_name.clone()),
                Box::new(err.to_string()),
            ];
            return Some(compiler::EmitResult {
                emit_skipped: true,
                diagnostics: vec![ast::new_compiler_diagnostic(
                    &diagnostics::Could_not_write_file_0_Colon_1,
                    &diagnostic_args,
                )],
                ..Default::default()
            });
        }
        self.snapshot
            .build_info_emit_pending
            .store(false, Ordering::SeqCst);
        Some(compiler::EmitResult {
            emit_skipped: false,
            emitted_files: vec![build_info_file_name],
            ..Default::default()
        })
    }

    pub fn ensure_has_errors_for_state(&mut self, ctx: core::Context, program: &compiler::Program) {
        let mut has_include_processing_diagnostics: Option<Box<dyn Fn() -> bool>> = None;
        let has_emit_diagnostics;
        if self.snapshot.can_use_incremental_state() {
            has_emit_diagnostics = program.get_source_files().iter().any(|file| {
                if self
                    .snapshot
                    .emit_diagnostics_per_file
                    .contains_key(&file.path())
                {
                    // emit diagnostics will be encoded in buildInfo;
                    return true;
                }
                if has_include_processing_diagnostics.is_none()
                    && !program.get_include_processor_diagnostics(file).is_empty()
                {
                    has_include_processing_diagnostics = Some(Box::new(|| true));
                }
                false
            });
            if has_include_processing_diagnostics.is_none() {
                has_include_processing_diagnostics = Some(Box::new(|| false));
            }
        } else {
            has_emit_diagnostics = self.snapshot.has_emit_diagnostics;
            let files = program.get_source_files();
            has_include_processing_diagnostics = Some(Box::new(move || {
                files
                    .iter()
                    .any(|file| !program.get_include_processor_diagnostics(file).is_empty())
            }));
        }

        if has_emit_diagnostics {
            // Record this for only non incremental build info
            self.snapshot.has_errors = core::if_else(
                self.snapshot.options.is_incremental(),
                core::TS_FALSE,
                core::TS_TRUE,
            );
            // Dont need to encode semantic errors state since the emit diagnostics are encoded
            self.snapshot.has_semantic_errors = false;
            return;
        }

        if has_include_processing_diagnostics.unwrap()()
            || !program.get_config_file_parsing_diagnostics().is_empty()
            || !program
                .get_syntactic_diagnostics(ctx.clone(), None)
                .is_empty()
            || !program.get_program_diagnostics().is_empty()
            || !program.get_global_diagnostics(ctx).is_empty()
        {
            self.snapshot.has_errors = core::TS_TRUE;
            // Dont need to encode semantic errors state since the syntax and program diagnostics are encoded as present
            self.snapshot.has_semantic_errors = false;
            return;
        }

        self.snapshot.has_errors = core::TS_FALSE;
        // Check semantic and emit diagnostics first as we dont need to ask program about it
        if program.get_source_files().iter().any(|file| {
            let semantic_diagnostics = self
                .snapshot
                .semantic_diagnostics_per_file
                .get(&file.path());
            if semantic_diagnostics.is_none() {
                // Missing semantic diagnostics in cache will be encoded in incremental buildInfo
                return self.snapshot.options.is_incremental();
            }
            let semantic_diagnostics = semantic_diagnostics.unwrap();
            if !semantic_diagnostics.diagnostics.is_empty()
                || !semantic_diagnostics.build_info_diagnostics.is_empty()
            {
                // cached semantic diagnostics will be encoded in buildInfo
                return true;
            }
            false
        }) {
            // Because semantic diagnostics are recorded in buildInfo, we dont need to encode hasErrors in incremental buildInfo
            // But encode as errors in non incremental buildInfo
            self.snapshot.has_semantic_errors = !self.snapshot.options.is_incremental();
        }
    }
}

impl compiler::ProgramLike for Program {
    fn options(&self) -> &core::CompilerOptions {
        &self.snapshot.options
    }

    fn get_source_file(&self, path: &str) -> Option<ast::SourceFile> {
        Program::get_source_file(self, path)
    }

    fn get_source_files(&self) -> Vec<ast::SourceFile> {
        Program::get_source_files(self)
    }

    fn get_parsed_source_files_refs(&self) -> Vec<&ast::SourceFile> {
        self.get_program().get_parsed_source_files_refs()
    }

    fn get_config_file_parsing_diagnostics(&self) -> Vec<ast::Diagnostic> {
        Program::get_config_file_parsing_diagnostics(self)
    }

    fn get_syntactic_diagnostics(
        &self,
        ctx: core::Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_syntactic_diagnostics(self, ctx, file.map(ast::SourceFile::share_readonly))
    }

    fn get_bind_diagnostics(
        &self,
        ctx: core::Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_bind_diagnostics(self, ctx, file.map(ast::SourceFile::share_readonly))
    }

    fn get_program_diagnostics(&self) -> Vec<ast::Diagnostic> {
        Program::get_program_diagnostics(self)
    }

    fn get_global_diagnostics(&self, ctx: core::Context) -> Vec<ast::Diagnostic> {
        Program::get_global_diagnostics(self, ctx)
    }

    fn get_semantic_diagnostics(
        &mut self,
        ctx: core::Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_semantic_diagnostics(self, ctx, file.map(ast::SourceFile::share_readonly))
    }

    fn get_declaration_diagnostics(
        &mut self,
        ctx: core::Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_declaration_diagnostics(self, ctx, file.map(ast::SourceFile::share_readonly))
    }

    fn get_suggestion_diagnostics(
        &self,
        ctx: core::Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_suggestion_diagnostics(self, ctx, file.map(ast::SourceFile::share_readonly))
    }

    fn emit(
        &mut self,
        ctx: core::Context,
        options: compiler::EmitOptions,
    ) -> Option<compiler::EmitResult> {
        Program::emit(self, ctx, options)
    }

    fn common_source_directory(&self) -> String {
        Program::common_source_directory(self)
    }

    fn is_source_file_default_library(&self, path: tspath::Path) -> bool {
        Program::is_source_file_default_library(self, path)
    }

    fn program(&self) -> &compiler::Program {
        Program::program(self)
    }
}
