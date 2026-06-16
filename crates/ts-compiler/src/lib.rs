#![forbid(unsafe_code)]
#![expect(
    non_snake_case,
    reason = "module names mirror upstream TypeScript-Go files"
)]
// The compiler port still mirrors TypeScript-Go's shared receiver mutation in
// several Program paths. Each raw mutation site carries the local invariant;
// the longer-term Rust shape should move those fields to interior mutability
// once this port stabilizes.

macro_rules! hashmap {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = std::collections::HashMap::new();
        $(map.insert($key.to_string(), $value.to_string());)*
        map
    }};
}

mod checkerpool;
#[path = "emitHost.rs"]
mod emit_host;
mod emitter;
#[path = "fileInclude.rs"]
mod file_include;
#[expect(
    dead_code,
    reason = "ported file loader helpers are ahead of current callers"
)]
mod fileloader;
#[expect(
    dead_code,
    reason = "ported file parsing task state is ahead of current callers"
)]
mod filesparser;
mod host;
mod includeprocessor;
mod processingDiagnostic;
#[expect(
    dead_code,
    private_interfaces,
    reason = "ported Program internals are ahead of current callers"
)]
mod program;
#[expect(
    dead_code,
    reason = "ported project-reference host is ahead of current callers"
)]
mod projectreferencedtsfakinghost;
mod projectreferencefilemapper;
mod projectreferenceparser;

pub use checkerpool::{
    ActiveChecker, CheckerAccess, CheckerCallback, CheckerPool,
    checker_slot_index_from_state_identity,
};
pub use emit_host::EmitHost;
pub use emitter::{
    EMIT_ALL, EMIT_ONLY_DTS, EMIT_ONLY_FORCED_DTS, EMIT_ONLY_JS, EMIT_ONLY_NONE, EmitOnly,
    SourceFileMayBeEmittedHost,
};
pub use file_include::FileIncludeReason;
pub use fileloader::{DuplicateSourceFile, LibFile};
pub use host::{
    CompilerHost, DiagnosticArgs, Trace, TraceText, new_cached_fs_compiler_host, new_compiler_host,
    new_compiler_host_with_text_trace,
};
pub use program::{
    CreateCheckerPool, EmitOptions, EmitResult, Program, ProgramLike, ProgramOptions,
    SourceMapEmitResult, WriteFileData, combine_emit_results, config_file_parsing_diagnostic,
    filter_no_emit_semantic_diagnostics, get_diagnostics_of_any_program, handle_no_emit_on_error,
    new_program, outputpaths_compiler_options, sort_and_deduplicate_diagnostics,
};

pub type WriteFile = Rc<
    RefCell<
        dyn for<'a, 'b, 'c> FnMut(
            &'a str,
            &'b str,
            Option<&'c mut WriteFileData>,
        ) -> Result<(), String>,
    >,
>;
pub(crate) use checkerpool::{CheckerPoolImpl, NullCheckerPool};

impl<T> CompilerHost for Box<T>
where
    T: CompilerHost + ?Sized,
{
    fn fs(&self) -> &dyn ts_vfs::Fs {
        (**self).fs()
    }

    fn default_library_path(&self) -> String {
        (**self).default_library_path()
    }

    fn get_current_directory(&self) -> String {
        (**self).get_current_directory()
    }

    fn trace(&self, msg: &'static ts_diagnostics::Message, args: &DiagnosticArgs) {
        (**self).trace(msg, args)
    }

    fn trace_text(&self, msg: &str) {
        (**self).trace_text(msg)
    }

    fn get_parsed_source_file(
        &self,
        opts: ts_ast::SourceFileParseOptions,
    ) -> Option<ts_ast::ParsedSourceFile> {
        (**self).get_parsed_source_file(opts)
    }

    fn get_source_file(&self, opts: ts_ast::SourceFileParseOptions) -> Option<ts_ast::SourceFile> {
        (**self).get_source_file(opts)
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: ts_tspath::Path,
    ) -> Option<ts_tsoptions::ParsedCommandLine> {
        (**self).get_resolved_project_reference(file_name, path)
    }
}

impl<T> CompilerHost for std::sync::Arc<T>
where
    T: CompilerHost + ?Sized,
{
    fn fs(&self) -> &dyn ts_vfs::Fs {
        (**self).fs()
    }

    fn default_library_path(&self) -> String {
        (**self).default_library_path()
    }

    fn get_current_directory(&self) -> String {
        (**self).get_current_directory()
    }

    fn trace(&self, msg: &'static ts_diagnostics::Message, args: &DiagnosticArgs) {
        (**self).trace(msg, args)
    }

    fn trace_text(&self, msg: &str) {
        (**self).trace_text(msg)
    }

    fn get_parsed_source_file(
        &self,
        opts: ts_ast::SourceFileParseOptions,
    ) -> Option<ts_ast::ParsedSourceFile> {
        (**self).get_parsed_source_file(opts)
    }

    fn get_source_file(&self, opts: ts_ast::SourceFileParseOptions) -> Option<ts_ast::SourceFile> {
        (**self).get_source_file(opts)
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: ts_tspath::Path,
    ) -> Option<ts_tsoptions::ParsedCommandLine> {
        (**self).get_resolved_project_reference(file_name, path)
    }
}

pub(crate) use emitter::source_file_may_be_emitted;
pub(crate) use file_include::{
    FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE, FILE_INCLUDE_KIND_IMPORT,
    FILE_INCLUDE_KIND_LIB_FILE, FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE,
    FILE_INCLUDE_KIND_REFERENCE_FILE, FILE_INCLUDE_KIND_ROOT_FILE,
    FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE,
};
pub(crate) use fileloader::{
    FileLoader, JsxRuntimeImportSpecifier, ProcessedFiles, ProcessedProgramFiles, RedirectsFile,
    get_default_resolution_mode_for_file, get_emit_syntax_for_usage_location_worker,
    get_mode_for_usage_location, process_all_program_files,
};
pub(crate) use includeprocessor::{IncludeProcessor, update_file_include_processor};
pub(crate) use processingDiagnostic::{
    IncludeExplainingDiagnostic, ProcessingDiagnostic, ProcessingDiagnosticData,
    ProcessingDiagnosticKind,
};
pub(crate) use projectreferencedtsfakinghost::{
    ProjectReferenceFileMapper, SourceOutputAndProjectReference,
    new_project_reference_dts_faking_host,
};
use std::{cell::RefCell, rc::Rc};
