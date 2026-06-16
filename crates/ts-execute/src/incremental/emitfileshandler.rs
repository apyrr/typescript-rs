use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_tspath as tspath;

use super::snapshot::{
    self, FILE_EMIT_KIND_ALL_DTS, FILE_EMIT_KIND_ALL_JS, FILE_EMIT_KIND_DTS_ERRORS,
};
use super::{
    DiagnosticsOrBuildInfoDiagnosticsWithFileName, FileEmitKind, MaybeProgramExt as _, Program,
    SIGNATURE_UPDATE_KIND_STORED_AT_EMIT, collect_all_affected_files,
};

pub struct EmitUpdate {
    pub pending_kind: FileEmitKind,
    pub result: Option<compiler::EmitResult>,
    pub dts_errors_from_cache: bool,
}

#[derive(Default)]
struct EmitWriteState {
    signatures: HashMap<tspath::Path, String>,
    emit_signatures: HashMap<tspath::Path, snapshot::EmitSignature>,
    latest_changed_dts_files: HashMap<tspath::Path, String>,
    has_emit_diagnostics: bool,
}

pub struct EmitFilesHandler<'a> {
    pub ctx: core::Context,
    pub program: &'a mut Program,
    pub is_for_dts_errors: bool,
    pub signatures: HashMap<tspath::Path, String>,
    pub emit_signatures: HashMap<tspath::Path, snapshot::EmitSignature>,
    pub latest_changed_dts_files: HashMap<tspath::Path, String>,
    pub deleted_pending_kinds: HashSet<tspath::Path>,
    pub emit_updates: HashMap<tspath::Path, EmitUpdate>,
    pub has_emit_diagnostics: AtomicBool,
    write_states: Vec<Rc<RefCell<EmitWriteState>>>,
}

impl<'a> EmitFilesHandler<'a> {
    // Determining what all is pending to be emitted based on previous options or previous file emit flags
    pub fn get_pending_emit_kind_for_emit_options(
        &self,
        emit_kind: FileEmitKind,
        options: compiler::EmitOptions,
    ) -> FileEmitKind {
        let mut pending_kind = get_pending_emit_kind(emit_kind, 0);
        if options.emit_only == compiler::EMIT_ONLY_DTS {
            pending_kind &= FILE_EMIT_KIND_ALL_DTS;
        }
        if self.is_for_dts_errors {
            pending_kind &= FILE_EMIT_KIND_DTS_ERRORS;
        }
        pending_kind
    }

    // Emits the next affected file's emit result (EmitResult and sourceFiles emitted) or returns undefined if iteration is complete
    // The first of writeFile if provided, writeFile of BuilderProgramHost if provided, writeFile of compiler host
    // in that order would be used to write the files
    pub fn emit_all_affected_files(
        &mut self,
        options: compiler::EmitOptions,
    ) -> Option<compiler::EmitResult> {
        // Emit all affected files
        if self.program.snapshot.can_use_incremental_state() {
            let results = self.emit_files_incremental(options.clone());
            if self.is_for_dts_errors {
                if let Some(target_source_file) = options.target_source_file {
                    // Result from cache
                    let mut diagnostics = self
                        .program
                        .snapshot
                        .emit_diagnostics_per_file
                        .get(&target_source_file.path())
                        .cloned()
                        .unwrap_or_default();
                    return Some(compiler::EmitResult {
                        emit_skipped: true,
                        diagnostics: diagnostics.get_diagnostics(
                            self.program.program.as_program(),
                            &target_source_file,
                        ),
                        ..Default::default()
                    });
                }
                return Some(compiler::combine_emit_results(results));
            } else {
                // Combine results and update buildInfo
                let mut result = compiler::combine_emit_results(results);
                self.emit_build_info(options, &mut result);
                return Some(result);
            }
        } else if !self.is_for_dts_errors {
            let emit_options = self.get_emit_options(options.clone());
            let mut result = self.program.program.emit(self.ctx.clone(), emit_options);
            self.update_snapshot();
            if let Some(result) = result.as_mut() {
                self.emit_build_info(options, result);
            }
            result
        } else {
            let result = compiler::EmitResult {
                emit_skipped: true,
                diagnostics: self
                    .program
                    .program
                    .get_declaration_diagnostics(self.ctx.clone(), options.target_source_file),
                ..Default::default()
            };
            if !result.diagnostics.is_empty() {
                self.program.snapshot.has_emit_diagnostics = true;
            }
            Some(result)
        }
    }

    pub fn emit_build_info(
        &mut self,
        options: compiler::EmitOptions,
        result: &mut compiler::EmitResult,
    ) {
        let build_info_result = self.program.emit_build_info(self.ctx.clone(), options);
        if let Some(build_info_result) = build_info_result {
            result.diagnostics.extend(build_info_result.diagnostics);
            result.emitted_files.extend(build_info_result.emitted_files);
        }
    }

    pub fn emit_files_incremental(
        &mut self,
        options: compiler::EmitOptions,
    ) -> Vec<compiler::EmitResult> {
        // Get all affected files
        collect_all_affected_files(self.ctx.clone(), self.program);
        if self.ctx.err().is_some() {
            return Vec::new();
        }

        for (path, emit_kind) in self.program.snapshot.affected_files_pending_emit.clone() {
            let affected_file = self.program.program.get_source_file_by_path(&path);
            if affected_file.is_none()
                || !self
                    .program
                    .program
                    .source_file_may_be_emitted(affected_file.as_ref().unwrap(), false)
            {
                self.deleted_pending_kinds.insert(path);
                continue;
            }
            let affected_file = affected_file.unwrap();
            let pending_kind =
                self.get_pending_emit_kind_for_emit_options(emit_kind, options.clone());
            if pending_kind != 0 {
                // PORT NOTE: reshaped for borrowck/thread-safety; current Rust
                // AST/checker state is not Send, so per-file emit runs inline.
                let mut emit_only = compiler::EMIT_ONLY_NONE;
                if (pending_kind & FILE_EMIT_KIND_ALL_JS) != 0 {
                    emit_only = compiler::EMIT_ONLY_JS;
                }
                if (pending_kind & FILE_EMIT_KIND_ALL_DTS) != 0 {
                    if emit_only == compiler::EMIT_ONLY_JS {
                        emit_only = compiler::EMIT_ALL;
                    } else {
                        emit_only = compiler::EMIT_ONLY_DTS;
                    }
                }
                let result = if !self.is_for_dts_errors {
                    let emit_options = self.get_emit_options(compiler::EmitOptions {
                        target_source_file: Some(affected_file.share_readonly()),
                        emit_only,
                        write_file: options.write_file.clone(),
                        ..Default::default()
                    });
                    self.program.program.emit(self.ctx.clone(), emit_options)
                } else {
                    Some(compiler::EmitResult {
                        emit_skipped: true,
                        diagnostics: self.program.program.get_declaration_diagnostics(
                            self.ctx.clone(),
                            Some(affected_file.share_readonly()),
                        ),
                        ..Default::default()
                    })
                };

                self.emit_updates.insert(
                    path.clone(),
                    EmitUpdate {
                        pending_kind: get_pending_emit_kind(emit_kind, pending_kind),
                        result,
                        dts_errors_from_cache: false,
                    },
                );
            }
        }
        if self.ctx.err().is_some() {
            return Vec::new();
        }

        // Get updated errors that were not included in affected files emit
        for (path, mut diagnostics) in self.program.snapshot.emit_diagnostics_per_file.clone() {
            if !self.emit_updates.contains_key(&path) {
                let affected_file = self.program.program.get_source_file_by_path(&path);
                if affected_file.is_none()
                    || !self
                        .program
                        .program
                        .source_file_may_be_emitted(affected_file.as_ref().unwrap(), false)
                {
                    self.deleted_pending_kinds.insert(path);
                    continue;
                }
                let pending_kind = self
                    .program
                    .snapshot
                    .affected_files_pending_emit
                    .get(&path)
                    .copied()
                    .unwrap_or_default();
                self.emit_updates.insert(
                    path.clone(),
                    EmitUpdate {
                        pending_kind,
                        result: Some(compiler::EmitResult {
                            emit_skipped: true,
                            diagnostics: diagnostics.get_diagnostics(
                                self.program.program.as_program(),
                                &affected_file.unwrap(),
                            ),
                            ..Default::default()
                        }),
                        dts_errors_from_cache: true,
                    },
                );
            }
        }

        self.update_snapshot()
    }

    pub fn get_emit_options(&mut self, options: compiler::EmitOptions) -> compiler::EmitOptions {
        if !self.program.snapshot.options.get_emit_declarations() {
            return options;
        }
        let can_use_incremental_state = self.program.snapshot.can_use_incremental_state();
        let target_source_file = options
            .target_source_file
            .as_ref()
            .map(ast::SourceFile::share_readonly);
        let closure_target_source_file = options
            .target_source_file
            .as_ref()
            .map(ast::SourceFile::share_readonly);
        let mut original_write_file = options.write_file.clone();
        let file_info = closure_target_source_file
            .as_ref()
            .and_then(|file| self.program.snapshot.file_infos.get(&file.path()).cloned());
        let old_emit_signature = closure_target_source_file.as_ref().and_then(|file| {
            self.program
                .snapshot
                .emit_signatures
                .get(&file.path())
                .cloned()
        });
        let options_for_emit = self.program.snapshot.options.clone();
        let hash_with_text = self.program.snapshot.hash_with_text;
        let compiler_host = self.program.program.as_program().host_arc();
        let state = Rc::new(RefCell::new(EmitWriteState::default()));
        self.write_states.push(Rc::clone(&state));
        compiler::EmitOptions {
            target_source_file,
            emit_only: options.emit_only,
            write_file: Some(Rc::new(RefCell::new(
                move |file_name: &str, text: &str, data: Option<&mut compiler::WriteFileData>| {
                    let mut default_data;
                    let data = if let Some(data) = data {
                        data
                    } else {
                        default_data = compiler::WriteFileData::default();
                        &mut default_data
                    };
                    let mut differs_only_in_map = false;
                    if tspath::is_declaration_file_name(file_name) {
                        if can_use_incremental_state {
                            let mut emit_signature = String::new();
                            let target_source_file = closure_target_source_file
                                .as_ref()
                                .expect("incremental declaration emit requires target source file");
                            let info = file_info
                                .as_ref()
                                .expect("incremental declaration emit requires file info");
                            if info.signature == info.version {
                                let signature = compute_signature_with_diagnostics(
                                    target_source_file,
                                    text,
                                    data,
                                    hash_with_text,
                                );
                                // With d.ts diagnostics they are also part of the signature so emitSignature
                                // will be different from it since its just hash of d.ts
                                if data.diagnostics.is_empty() {
                                    emit_signature = signature.clone();
                                }
                                if signature != info.version {
                                    state
                                        .borrow_mut()
                                        .signatures
                                        .insert(target_source_file.path(), signature);
                                }
                            }

                            // Store d.ts emit hash so later can be compared to check if d.ts has changed.
                            // Currently we do this only for composite projects since these are the only
                            // projects that can be referenced by other projects and would need their d.ts
                            // change time in --build mode
                            if skip_dts_output_of_composite(
                                &mut state.borrow_mut(),
                                &options_for_emit,
                                old_emit_signature.as_ref(),
                                hash_with_text,
                                target_source_file,
                                file_name,
                                text,
                                data,
                                emit_signature,
                                &mut differs_only_in_map,
                            ) {
                                return Ok(());
                            }
                        } else if !data.diagnostics.is_empty() {
                            state.borrow_mut().has_emit_diagnostics = true;
                        }
                    }

                    let a_time = if differs_only_in_map {
                        compiler_host
                            .fs()
                            .stat(file_name)
                            .ok()
                            .and_then(|info| info.modified().ok())
                    } else {
                        None
                    };
                    let err = if let Some(write_file) = original_write_file.as_mut() {
                        (write_file.borrow_mut())(file_name, text, Some(data))
                    } else {
                        compiler_host
                            .fs()
                            .write_file(file_name, text)
                            .map_err(|err| err.to_string())
                    };
                    if err.is_ok() {
                        if let Some(a_time) = a_time {
                            return compiler_host
                                .fs()
                                .chtimes(file_name, std::time::SystemTime::UNIX_EPOCH, a_time)
                                .map_err(|err| err.to_string());
                        }
                    }
                    err
                },
            ))),
        }
    }

    // Compare to existing computed signature and store it or handle the changes in d.ts map option from before
    // returning undefined means that, we dont need to emit this d.ts file since its contents didnt change
    pub fn skip_dts_output_of_composite(
        &mut self,
        file: &ast::SourceFile,
        output_file_name: &str,
        text: &str,
        data: &mut compiler::WriteFileData,
        mut new_signature: String,
        differs_only_in_map: &mut bool,
    ) -> bool {
        if !self.program.snapshot.options.composite.is_true() {
            return false;
        }
        let mut old_signature = String::new();
        let old_signature_format = self
            .program
            .snapshot
            .emit_signatures
            .get(&file.path())
            .cloned();
        if let Some(old_signature_format) = &old_signature_format {
            if !old_signature_format.signature.is_empty() {
                old_signature = old_signature_format.signature.clone();
            } else {
                old_signature = old_signature_format.signature_with_different_options[0].clone();
            }
        }
        if new_signature.is_empty() {
            new_signature = self
                .program
                .snapshot
                .compute_hash(&get_text_handling_source_map_for_signature(text, data));
        }
        // Dont write dts files if they didn't change
        if new_signature == old_signature {
            // If the signature was encoded as string the dts map options match so nothing to do
            if old_signature_format
                .as_ref()
                .is_some_and(|format| format.signature == old_signature)
            {
                data.skipped_dts_write = true;
                return true;
            } else {
                // Mark as differsOnlyInMap so that we can reverse the timestamp with --build so that
                // the downstream projects dont detect this as change in d.ts file
                *differs_only_in_map = self.program.options().build.is_true();
            }
        } else {
            self.latest_changed_dts_files
                .insert(file.path(), output_file_name.to_owned());
        }
        self.emit_signatures.insert(
            file.path(),
            snapshot::EmitSignature {
                signature: new_signature,
                signature_with_different_options: Vec::new(),
            },
        );
        false
    }

    pub fn update_snapshot(&mut self) -> Vec<compiler::EmitResult> {
        for state in self.write_states.drain(..) {
            let state = state.borrow();
            self.signatures.extend(state.signatures.clone());
            self.emit_signatures.extend(state.emit_signatures.clone());
            self.latest_changed_dts_files
                .extend(state.latest_changed_dts_files.clone());
            if state.has_emit_diagnostics {
                self.has_emit_diagnostics.store(true, Ordering::SeqCst);
            }
        }
        if self.program.snapshot.can_use_incremental_state() {
            for (file, signature) in &self.signatures {
                if let Some(info) = self.program.snapshot.file_infos.get_mut(file) {
                    info.signature = signature.clone();
                    if let Some(testing_data) = &mut self.program.testing_data {
                        testing_data
                            .updated_signature_kinds
                            .insert(file.clone(), SIGNATURE_UPDATE_KIND_STORED_AT_EMIT);
                    }
                    self.program
                        .snapshot
                        .build_info_emit_pending
                        .store(true, Ordering::SeqCst);
                }
            }
            for (file, signature) in &self.emit_signatures {
                self.program
                    .snapshot
                    .emit_signatures
                    .insert(file.clone(), signature.clone());
                self.program
                    .snapshot
                    .build_info_emit_pending
                    .store(true, Ordering::SeqCst);
            }
            for file in &self.deleted_pending_kinds {
                self.program
                    .snapshot
                    .affected_files_pending_emit
                    .remove(file);
                self.program
                    .snapshot
                    .build_info_emit_pending
                    .store(true, Ordering::SeqCst);
            }
            // Always use correct order when to collect the result
            let mut results = Vec::new();
            for file in self.program.get_source_files() {
                if let Some(latest_changed_dts_file) =
                    self.latest_changed_dts_files.get(&file.path()).cloned()
                {
                    self.program.snapshot.latest_changed_dts_file = latest_changed_dts_file;
                    self.program
                        .snapshot
                        .build_info_emit_pending
                        .store(true, Ordering::SeqCst);
                    self.program.snapshot.has_changed_dts_file = true;
                }
                if let Some(update) = self.emit_updates.get(&file.path()) {
                    if !update.dts_errors_from_cache {
                        if update.pending_kind == 0 {
                            self.program
                                .snapshot
                                .affected_files_pending_emit
                                .remove(&file.path());
                        } else {
                            self.program
                                .snapshot
                                .affected_files_pending_emit
                                .insert(file.path(), update.pending_kind);
                        }
                        self.program
                            .snapshot
                            .build_info_emit_pending
                            .store(true, Ordering::SeqCst);
                    }
                    if let Some(result) = &update.result {
                        results.push(result.clone());
                        if !result.diagnostics.is_empty() {
                            self.program.snapshot.emit_diagnostics_per_file.insert(
                                file.path(),
                                DiagnosticsOrBuildInfoDiagnosticsWithFileName {
                                    diagnostics: result.diagnostics.clone(),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }
            return results;
        } else if self.has_emit_diagnostics.load(Ordering::SeqCst) {
            self.program.snapshot.has_emit_diagnostics = true;
        }
        Vec::new()
    }
}

pub fn get_pending_emit_kind(emit_kind: FileEmitKind, emitted_kind: FileEmitKind) -> FileEmitKind {
    snapshot::get_pending_emit_kind(emit_kind, emitted_kind)
}

pub fn get_text_handling_source_map_for_signature(
    text: &str,
    data: &compiler::WriteFileData,
) -> String {
    if data.source_map_url_pos != -1 {
        return text[..data.source_map_url_pos as usize].to_owned();
    }
    text.to_owned()
}

fn compute_signature_with_diagnostics(
    file: &ast::SourceFile,
    text: &str,
    data: &compiler::WriteFileData,
    hash_with_text: bool,
) -> String {
    let mut builder = String::new();
    builder.push_str(&get_text_handling_source_map_for_signature(text, data));
    for diag in &data.diagnostics {
        snapshot::diagnostic_to_string_builder(diag, file, &mut builder);
    }
    snapshot::compute_hash(&builder, hash_with_text)
}

fn skip_dts_output_of_composite(
    state: &mut EmitWriteState,
    options: &core::CompilerOptions,
    old_signature_format: Option<&snapshot::EmitSignature>,
    hash_with_text: bool,
    file: &ast::SourceFile,
    output_file_name: &str,
    text: &str,
    data: &mut compiler::WriteFileData,
    mut new_signature: String,
    differs_only_in_map: &mut bool,
) -> bool {
    if !options.composite.is_true() {
        return false;
    }
    let mut old_signature = String::new();
    if let Some(old_signature_format) = old_signature_format {
        if !old_signature_format.signature.is_empty() {
            old_signature = old_signature_format.signature.clone();
        } else {
            old_signature = old_signature_format.signature_with_different_options[0].clone();
        }
    }
    if new_signature.is_empty() {
        new_signature = snapshot::compute_hash(
            &get_text_handling_source_map_for_signature(text, data),
            hash_with_text,
        );
    }
    if new_signature == old_signature {
        if old_signature_format.is_some_and(|format| format.signature == old_signature) {
            data.skipped_dts_write = true;
            return true;
        }
        *differs_only_in_map = options.build.is_true();
    } else {
        state
            .latest_changed_dts_files
            .insert(file.path(), output_file_name.to_owned());
    }
    state.emit_signatures.insert(
        file.path(),
        snapshot::EmitSignature {
            signature: new_signature,
            signature_with_different_options: Vec::new(),
        },
    );
    false
}

pub fn emit_files(
    ctx: core::Context,
    program: &mut Program,
    options: compiler::EmitOptions,
    is_for_dts_errors: bool,
) -> Option<compiler::EmitResult> {
    let mut emit_handler = EmitFilesHandler {
        ctx: ctx.clone(),
        program,
        is_for_dts_errors,
        signatures: HashMap::new(),
        emit_signatures: HashMap::new(),
        latest_changed_dts_files: HashMap::new(),
        deleted_pending_kinds: HashSet::new(),
        emit_updates: HashMap::new(),
        has_emit_diagnostics: AtomicBool::new(false),
        write_states: Vec::new(),
    };

    // Single file emit - do direct from program
    if !is_for_dts_errors && options.target_source_file.is_some() {
        let emit_options = emit_handler.get_emit_options(options);
        let result = emit_handler.program.program.emit(ctx.clone(), emit_options);
        if ctx.err().is_some() {
            return None;
        }
        emit_handler.update_snapshot();
        return result;
    }

    // Emit only affected files if using builder for emit
    emit_handler.emit_all_affected_files(options)
}
