use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tspath as tspath;

#[derive(Clone, Default)]
pub struct FileInfo {
    pub version: String,
    pub signature: String,
    pub affects_global_scope: bool,
    pub implied_node_format: core::ResolutionMode,
}

impl FileInfo {
    pub fn version(&self) -> String {
        self.version.clone()
    }

    pub fn signature(&self) -> String {
        self.signature.clone()
    }

    pub fn affects_global_scope(&self) -> bool {
        self.affects_global_scope
    }

    pub fn implied_node_format(&self) -> core::ResolutionMode {
        self.implied_node_format
    }
}

pub fn compute_hash(text: &str, hash_with_text: bool) -> String {
    let hash = format!("{:032x}", xxhash_rust::xxh3::xxh3_128(text.as_bytes()));
    if hash_with_text {
        return format!("{hash}-{text}");
    }
    hash
}

pub type FileEmitKind = u32;

pub const FILE_EMIT_KIND_NONE: FileEmitKind = 0;
pub const FILE_EMIT_KIND_JS: FileEmitKind = 1 << 0; // emit js file
pub const FILE_EMIT_KIND_JS_MAP: FileEmitKind = 1 << 1; // emit js.map file
pub const FILE_EMIT_KIND_JS_INLINE_MAP: FileEmitKind = 1 << 2; // emit inline source map in js file
pub const FILE_EMIT_KIND_DTS_ERRORS: FileEmitKind = 1 << 3; // emit dts errors
pub const FILE_EMIT_KIND_DTS_EMIT: FileEmitKind = 1 << 4; // emit d.ts file
pub const FILE_EMIT_KIND_DTS_MAP: FileEmitKind = 1 << 5; // emit d.ts.map file

pub const FILE_EMIT_KIND_DTS: FileEmitKind = FILE_EMIT_KIND_DTS_ERRORS | FILE_EMIT_KIND_DTS_EMIT;
pub const FILE_EMIT_KIND_ALL_JS: FileEmitKind =
    FILE_EMIT_KIND_JS | FILE_EMIT_KIND_JS_MAP | FILE_EMIT_KIND_JS_INLINE_MAP;
pub const FILE_EMIT_KIND_ALL_DTS_EMIT: FileEmitKind =
    FILE_EMIT_KIND_DTS_EMIT | FILE_EMIT_KIND_DTS_MAP;
pub const FILE_EMIT_KIND_ALL_DTS: FileEmitKind = FILE_EMIT_KIND_DTS | FILE_EMIT_KIND_DTS_MAP;
pub const FILE_EMIT_KIND_ALL: FileEmitKind = FILE_EMIT_KIND_ALL_JS | FILE_EMIT_KIND_ALL_DTS;

pub fn get_file_emit_kind(options: core::CompilerOptions) -> FileEmitKind {
    let mut result = FILE_EMIT_KIND_JS;
    if options.source_map.is_true() {
        result |= FILE_EMIT_KIND_JS_MAP;
    }
    if options.inline_source_map.is_true() {
        result |= FILE_EMIT_KIND_JS_INLINE_MAP;
    }
    if options.get_emit_declarations() {
        result |= FILE_EMIT_KIND_DTS;
    }
    if options.declaration_map.is_true() {
        result |= FILE_EMIT_KIND_DTS_MAP;
    }
    if options.emit_declaration_only.is_true() {
        result &= FILE_EMIT_KIND_ALL_DTS;
    }
    result
}

pub fn get_pending_emit_kind_with_options(
    options: core::CompilerOptions,
    old_options: core::CompilerOptions,
) -> FileEmitKind {
    let old_emit_kind = get_file_emit_kind(old_options);
    let new_emit_kind = get_file_emit_kind(options);
    get_pending_emit_kind(new_emit_kind, old_emit_kind)
}

pub fn get_pending_emit_kind(emit_kind: FileEmitKind, old_emit_kind: FileEmitKind) -> FileEmitKind {
    if old_emit_kind == emit_kind {
        return FILE_EMIT_KIND_NONE;
    }
    if old_emit_kind == 0 || emit_kind == 0 {
        return emit_kind;
    }
    let diff = old_emit_kind ^ emit_kind;
    let mut result = FILE_EMIT_KIND_NONE;
    // If there is diff in Js emit, pending emit is js emit flags
    if (diff & FILE_EMIT_KIND_ALL_JS) != 0 {
        result |= emit_kind & FILE_EMIT_KIND_ALL_JS;
    }
    // If dts errors pending, add dts errors flag
    if (diff & FILE_EMIT_KIND_DTS_ERRORS) != 0 {
        result |= emit_kind & FILE_EMIT_KIND_ALL_DTS;
    }
    // If there is diff in Dts emit, pending emit is dts emit flags
    if (diff & FILE_EMIT_KIND_ALL_DTS_EMIT) != 0 {
        result |= emit_kind & FILE_EMIT_KIND_ALL_DTS_EMIT;
    }
    result
}

// Signature (Hash of d.ts emitted), is string if it was emitted using same d.ts.map option as what compilerOptions indicate,
// otherwise tuple of string
#[derive(Clone, Default)]
pub struct EmitSignature {
    pub signature: String,
    pub signature_with_different_options: Vec<String>,
}

impl EmitSignature {
    // Covert to Emit signature based on oldOptions and EmitSignature format
    // If d.ts map options differ then swap the format, otherwise use as is
    pub fn get_new_emit_signature(
        &self,
        old_options: core::CompilerOptions,
        new_options: core::CompilerOptions,
    ) -> EmitSignature {
        if old_options.declaration_map.is_true() == new_options.declaration_map.is_true() {
            return self.clone();
        }
        if self.signature_with_different_options.is_empty() {
            EmitSignature {
                signature_with_different_options: vec![self.signature.clone()],
                signature: String::new(),
            }
        } else {
            EmitSignature {
                signature: self.signature_with_different_options[0].clone(),
                signature_with_different_options: Vec::new(),
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct BuildInfoDiagnosticWithFileName {
    // filename if it is for a File thats other than its stored for
    pub file: tspath::Path,
    pub no_file: bool,
    pub pos: i32,
    pub end: i32,
    pub code: i32,
    pub category: diagnostics::Category,
    pub message_key: diagnostics::Key,
    pub message_args: Vec<String>,
    pub message_chain: Vec<BuildInfoDiagnosticWithFileName>,
    pub related_information: Vec<BuildInfoDiagnosticWithFileName>,
    pub reports_unnecessary: bool,
    pub reports_deprecated: bool,
    pub skipped_on_no_emit: bool,
    pub repopulate_info: Option<ast::RepopulateDiagnosticInfo>,
}

#[derive(Clone, Default)]
pub struct DiagnosticsOrBuildInfoDiagnosticsWithFileName {
    pub diagnostics: Vec<ast::Diagnostic>,
    pub build_info_diagnostics: Vec<BuildInfoDiagnosticWithFileName>,
}

impl BuildInfoDiagnosticWithFileName {
    pub fn to_diagnostic(
        &self,
        p: &compiler::Program,
        file: Option<&ast::SourceFile>,
    ) -> ast::Diagnostic {
        let file_for_diagnostic = if !self.file.is_empty() {
            p.get_source_file_by_path(self.file.clone())
        } else if !self.no_file {
            file.map(ast::SourceFile::share_readonly)
        } else {
            None
        };

        if self.repopulate_info.is_some() {
            return repopulate_diagnostic_chain(self, p, file_for_diagnostic.as_ref());
        }

        let message_chain = self
            .message_chain
            .iter()
            .map(|msg| msg.to_diagnostic(p, file_for_diagnostic.as_ref()))
            .collect();
        let related_information = self
            .related_information
            .iter()
            .map(|info| info.to_diagnostic(p, file_for_diagnostic.as_ref()))
            .collect();
        ast::new_diagnostic_from_serialized(ast::SerializedDiagnosticParams {
            file: file_for_diagnostic
                .as_ref()
                .map(|file| file.diagnostic_file()),
            loc: core::new_text_range(self.pos, self.end),
            code: self.code,
            category: self.category,
            message_key: self.message_key.clone(),
            message_args: self.message_args.clone(),
            message_chain,
            related_information,
            reports_unnecessary: self.reports_unnecessary,
            reports_deprecated: self.reports_deprecated,
            skipped_on_no_emit: self.skipped_on_no_emit,
        })
    }

    pub fn to_diagnostic_without_repopulate(
        &self,
        p: &compiler::Program,
        file: Option<&ast::SourceFile>,
    ) -> ast::Diagnostic {
        let message_chain = self
            .message_chain
            .iter()
            .map(|msg| msg.to_diagnostic(p, file))
            .collect();
        let related_information = self
            .related_information
            .iter()
            .map(|info| info.to_diagnostic(p, file))
            .collect();
        ast::new_diagnostic_from_serialized(ast::SerializedDiagnosticParams {
            file: file.map(ast::DiagnosticFile::from_source_file),
            loc: core::new_text_range(self.pos, self.end),
            code: self.code,
            category: self.category,
            message_key: self.message_key.clone(),
            message_args: self.message_args.clone(),
            message_chain,
            related_information,
            reports_unnecessary: self.reports_unnecessary,
            reports_deprecated: self.reports_deprecated,
            skipped_on_no_emit: self.skipped_on_no_emit,
        })
    }
}

impl PartialEq for DiagnosticsOrBuildInfoDiagnosticsWithFileName {
    fn eq(&self, other: &Self) -> bool {
        fn diagnostic_eq(left: &ast::Diagnostic, right: &ast::Diagnostic) -> bool {
            left.file() == right.file()
                && left.loc() == right.loc()
                && left.code() == right.code()
                && left.category() == right.category()
                && left.message_key() == right.message_key()
                && left.message_args() == right.message_args()
                && left
                    .message_chain()
                    .iter()
                    .zip(right.message_chain())
                    .all(|(left, right)| diagnostic_eq(left, right))
                && left.message_chain().len() == right.message_chain().len()
        }

        self.build_info_diagnostics.len() == other.build_info_diagnostics.len()
            && self.diagnostics.len() == other.diagnostics.len()
            && self
                .diagnostics
                .iter()
                .zip(&other.diagnostics)
                .all(|(left, right)| diagnostic_eq(left, right))
    }
}

// repopulateDiagnosticChain recomputes a diagnostic chain entry that depends on
// program state which may have changed between incremental builds.
pub fn repopulate_diagnostic_chain(
    b: &BuildInfoDiagnosticWithFileName,
    p: &compiler::Program,
    file: Option<&ast::SourceFile>,
) -> ast::Diagnostic {
    let info = b.repopulate_info.as_ref().unwrap();
    match info.kind {
        ast::RepopulateDiagnosticKind::ModeMismatch => repopulate_mode_mismatch_chain(b, p, file),
        ast::RepopulateDiagnosticKind::ModuleNotFound => {
            repopulate_module_not_found_chain(b, p, file, info)
        }
        _ => {
            // Fall back to using the stored (possibly stale) data
            b.to_diagnostic_without_repopulate(p, file)
        }
    }
}

pub fn repopulate_mode_mismatch_chain(
    b: &BuildInfoDiagnosticWithFileName,
    p: &compiler::Program,
    file: Option<&ast::SourceFile>,
) -> ast::Diagnostic {
    let Some(file) = file else {
        return b.to_diagnostic_without_repopulate(p, file);
    };

    let details = checker::create_mode_mismatch_details(p, file);

    let next_chain = b
        .message_chain
        .iter()
        .map(|msg| msg.to_diagnostic(p, Some(file)))
        .collect();

    ast::new_diagnostic_from_serialized(ast::SerializedDiagnosticParams {
        file: Some(file.diagnostic_file()),
        loc: core::new_text_range(b.pos, b.end),
        code: details.message.code(),
        category: details.message.category(),
        message_key: details.message.key().clone(),
        message_args: details
            .args
            .into_iter()
            .map(|arg| arg.to_string())
            .collect(),
        message_chain: next_chain,
        related_information: Vec::new(),
        reports_unnecessary: false,
        reports_deprecated: false,
        skipped_on_no_emit: false,
    })
}

pub fn repopulate_module_not_found_chain(
    b: &BuildInfoDiagnosticWithFileName,
    p: &compiler::Program,
    file: Option<&ast::SourceFile>,
    info: &ast::RepopulateDiagnosticInfo,
) -> ast::Diagnostic {
    let Some(file) = file else {
        return b.to_diagnostic_without_repopulate(p, file);
    };

    let mut package_name = info.package_name.clone();
    if package_name.is_empty() {
        package_name = info.module_reference.clone();
    }

    let details = checker::create_module_not_found_chain(
        p,
        file,
        &info.module_reference,
        info.mode,
        package_name,
    );

    let next_chain = b
        .message_chain
        .iter()
        .map(|msg| msg.to_diagnostic(p, Some(file)))
        .collect();

    ast::new_diagnostic_from_serialized(ast::SerializedDiagnosticParams {
        file: Some(file.diagnostic_file()),
        loc: core::new_text_range(b.pos, b.end),
        code: details.message.code(),
        category: details.message.category(),
        message_key: details.message.key().clone(),
        message_args: details
            .args
            .into_iter()
            .map(|arg| arg.to_string())
            .collect(),
        message_chain: next_chain,
        related_information: Vec::new(),
        reports_unnecessary: false,
        reports_deprecated: false,
        skipped_on_no_emit: false,
    })
}

impl DiagnosticsOrBuildInfoDiagnosticsWithFileName {
    pub fn get_diagnostics(
        &mut self,
        p: &compiler::Program,
        file: &ast::SourceFile,
    ) -> Vec<ast::Diagnostic> {
        if !self.diagnostics.is_empty() {
            return self.diagnostics.clone();
        }
        // Convert and cache the diagnostics
        self.diagnostics = core::map(&self.build_info_diagnostics, |diag| {
            diag.to_diagnostic(p, Some(file))
        });
        self.diagnostics.clone()
    }
}

pub struct Snapshot {
    // These are the fields that get serialized

    // Information of the file eg. its version, signature etc
    pub file_infos: HashMap<tspath::Path, FileInfo>,
    pub options: core::CompilerOptions,
    //  Contains the map of ReferencedSet=Referenced files of the file if module emit is enabled
    pub referenced_map: ReferenceMap,
    // Cache of semantic diagnostics for files with their Path being the key
    pub semantic_diagnostics_per_file:
        HashMap<tspath::Path, DiagnosticsOrBuildInfoDiagnosticsWithFileName>,
    // Cache of dts emit diagnostics for files with their Path being the key
    pub emit_diagnostics_per_file:
        HashMap<tspath::Path, DiagnosticsOrBuildInfoDiagnosticsWithFileName>,
    // The map has key by source file's path that has been changed
    pub changed_files_set: HashSet<tspath::Path>,
    // Files pending to be emitted
    pub affected_files_pending_emit: HashMap<tspath::Path, FileEmitKind>,
    // Name of the file whose dts was the latest to change
    pub latest_changed_dts_file: String,
    // Hash of d.ts emitted for the file, use to track when emit of d.ts changes
    pub emit_signatures: HashMap<tspath::Path, EmitSignature>,
    // Recorded if program had errors that need to be reported even with --noCheck
    pub has_errors: core::Tristate,
    // Recorded if program had semantic errors only for non incremental build
    pub has_semantic_errors: bool,
    // If semantic diagnostic check is pending
    pub check_pending: bool,

    // Additional fields that are not serialized but needed to track state

    // true if build info emit is pending
    pub build_info_emit_pending: AtomicBool,
    pub has_errors_from_old_state: core::Tristate,
    pub has_semantic_errors_from_old_state: bool,
    pub all_files_excluding_default_library_file_once: bool,
    //  Cache of all files excluding default library file for the current program
    pub all_files_excluding_default_library_file: Vec<ast::SourceFile>,
    pub has_changed_dts_file: bool,
    pub has_emit_diagnostics: bool,

    // Used with testing to add text of hash for better comparison
    pub hash_with_text: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            file_infos: HashMap::new(),
            options: core::CompilerOptions::default(),
            referenced_map: ReferenceMap::default(),
            semantic_diagnostics_per_file: HashMap::new(),
            emit_diagnostics_per_file: HashMap::new(),
            changed_files_set: HashSet::new(),
            affected_files_pending_emit: HashMap::new(),
            latest_changed_dts_file: String::new(),
            emit_signatures: HashMap::new(),
            has_errors: core::TS_UNKNOWN,
            has_semantic_errors: false,
            check_pending: false,
            build_info_emit_pending: AtomicBool::new(false),
            has_errors_from_old_state: core::TS_UNKNOWN,
            has_semantic_errors_from_old_state: false,
            all_files_excluding_default_library_file_once: false,
            all_files_excluding_default_library_file: Vec::new(),
            has_changed_dts_file: false,
            has_emit_diagnostics: false,
            hash_with_text: false,
        }
    }
}

impl Clone for Snapshot {
    fn clone(&self) -> Self {
        Self {
            file_infos: self.file_infos.clone(),
            options: self.options.clone(),
            referenced_map: self.referenced_map.clone(),
            semantic_diagnostics_per_file: self.semantic_diagnostics_per_file.clone(),
            emit_diagnostics_per_file: self.emit_diagnostics_per_file.clone(),
            changed_files_set: self.changed_files_set.clone(),
            affected_files_pending_emit: self.affected_files_pending_emit.clone(),
            latest_changed_dts_file: self.latest_changed_dts_file.clone(),
            emit_signatures: self.emit_signatures.clone(),
            has_errors: self.has_errors,
            has_semantic_errors: self.has_semantic_errors,
            check_pending: self.check_pending,
            build_info_emit_pending: AtomicBool::new(
                self.build_info_emit_pending.load(Ordering::SeqCst),
            ),
            has_errors_from_old_state: self.has_errors_from_old_state,
            has_semantic_errors_from_old_state: self.has_semantic_errors_from_old_state,
            all_files_excluding_default_library_file_once: self
                .all_files_excluding_default_library_file_once,
            all_files_excluding_default_library_file: self
                .all_files_excluding_default_library_file
                .iter()
                .map(ast::SourceFile::share_readonly)
                .collect(),
            has_changed_dts_file: self.has_changed_dts_file,
            has_emit_diagnostics: self.has_emit_diagnostics,
            hash_with_text: self.hash_with_text,
        }
    }
}

impl Snapshot {
    pub fn add_file_to_change_set(&mut self, file_path: tspath::Path) {
        self.changed_files_set.insert(file_path);
        self.build_info_emit_pending.store(true, Ordering::SeqCst);
    }

    pub fn add_file_to_affected_files_pending_emit(
        &mut self,
        file_path: tspath::Path,
        emit_kind: FileEmitKind,
    ) {
        let existing_kind = self
            .affected_files_pending_emit
            .get(&file_path)
            .copied()
            .unwrap_or_default();
        self.affected_files_pending_emit
            .insert(file_path.clone(), existing_kind | emit_kind);
        if emit_kind & FILE_EMIT_KIND_DTS_ERRORS != 0 {
            self.emit_diagnostics_per_file.remove(&file_path);
        }
        self.build_info_emit_pending.store(true, Ordering::SeqCst);
    }

    pub fn get_all_files_excluding_default_library_file(
        &mut self,
        program: &compiler::Program,
        first_source_file: Option<ast::SourceFile>,
    ) -> Vec<ast::SourceFile> {
        if !self.all_files_excluding_default_library_file_once {
            self.all_files_excluding_default_library_file_once = true;
            let files = program.get_source_files();
            self.all_files_excluding_default_library_file = Vec::with_capacity(files.len());
            let mut add_source_file = |file: ast::SourceFile| {
                if !program.is_source_file_default_library(file.path()) {
                    self.all_files_excluding_default_library_file.push(file);
                }
            };
            let first_source_file_path = first_source_file.as_ref().map(ast::SourceFile::path);
            if let Some(first_source_file) = first_source_file {
                add_source_file(first_source_file);
            }
            for file in files {
                if first_source_file_path
                    .as_ref()
                    .is_none_or(|first| first != &file.path())
                {
                    add_source_file(file);
                }
            }
        }
        ast::SourceFile::share_readonly_slice(&self.all_files_excluding_default_library_file)
    }

    pub fn compute_signature_with_diagnostics(
        &self,
        file: &ast::SourceFile,
        text: &str,
        data: &compiler::WriteFileData,
    ) -> String {
        let mut builder = String::new();
        builder.push_str(&get_text_handling_source_map_for_signature(text, data));
        for diag in &data.diagnostics {
            diagnostic_to_string_builder(diag, file, &mut builder);
        }
        self.compute_hash(&builder)
    }

    pub fn compute_hash(&self, text: &str) -> String {
        compute_hash(text, self.hash_with_text)
    }

    pub fn can_use_incremental_state(&self) -> bool {
        if !self.options.is_incremental() && self.options.build.is_true() {
            // If not incremental build (with tsc -b), we don't need to track state except diagnostics per file so we can use it
            return false;
        }
        true
    }
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

pub fn diagnostic_to_string_builder(
    diagnostic: &ast::Diagnostic,
    file: &ast::SourceFile,
    builder: &mut String,
) {
    builder.push('\n');
    let file_path = file.path();
    if diagnostic.file().map(|file| file.path()) != Some(&file_path) {
        let diagnostic_file = diagnostic
            .file()
            .expect("diagnostic file must be present when it differs from the source file");
        builder.push_str(&tspath::ensure_path_is_non_module_name(
            &tspath::get_relative_path_from_directory(
                &tspath::get_directory_path(&file_path),
                diagnostic_file.path(),
                &tspath::ComparePathsOptions::default(),
            ),
        ));
    }
    if diagnostic.file().is_some() {
        let _ = write!(builder, "({},{}): ", diagnostic.pos(), diagnostic.len());
    }
    builder.push_str(&diagnostic.category().name());
    let _ = write!(builder, "{}: ", diagnostic.code());
    builder.push_str(&diagnostic.message_key().to_string());
    builder.push('\n');
    for arg in diagnostic.message_args() {
        builder.push_str(&arg);
        builder.push('\n');
    }
    for chain in diagnostic.message_chain() {
        diagnostic_to_string_builder(&chain, file, builder);
    }
    for info in diagnostic.related_information() {
        diagnostic_to_string_builder(&info, file, builder);
    }
}

#[derive(Clone, Default)]
pub struct ReferenceMap {
    pub references: HashMap<tspath::Path, HashSet<tspath::Path>>,
    referenced_by: OnceLock<HashMap<tspath::Path, HashSet<tspath::Path>>>,
}

impl ReferenceMap {
    pub fn store_references(&mut self, file: tspath::Path, refs: HashSet<tspath::Path>) {
        self.references.insert(file, refs);
    }

    pub fn get_references(&self, file: tspath::Path) -> HashSet<tspath::Path> {
        self.references.get(&file).cloned().unwrap_or_default()
    }

    pub fn get_paths_with_references(&self) -> Vec<tspath::Path> {
        self.references.keys().cloned().collect()
    }

    pub fn get_referenced_by(&self, file: tspath::Path) -> Vec<tspath::Path> {
        let referenced_by = self.referenced_by.get_or_init(|| {
            let mut referenced_by: HashMap<tspath::Path, HashSet<tspath::Path>> = HashMap::new();
            for (key, value) in &self.references {
                for r#ref in value {
                    referenced_by
                        .entry(r#ref.clone())
                        .or_default()
                        .insert(key.clone());
                }
            }
            referenced_by
        });
        referenced_by
            .get(&file)
            .map(|refs| refs.iter().cloned().collect())
            .unwrap_or_default()
    }
}
