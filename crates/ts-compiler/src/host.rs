use std::sync::Arc;

use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_parser as parser;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs as vfs;

pub type DiagnosticArgs = [diagnostics::Any];
pub type Trace = dyn Fn(&'static diagnostics::Message, &DiagnosticArgs) + Send + Sync;
pub type TraceText = dyn Fn(&str) + Send + Sync;

pub trait CompilerHost: Send + Sync {
    fn fs(&self) -> &dyn vfs::Fs;
    fn default_library_path(&self) -> String;
    fn get_current_directory(&self) -> String;
    fn trace(&self, msg: &'static diagnostics::Message, args: &DiagnosticArgs);
    fn trace_text(&self, msg: &str) {
        let _ = msg;
    }
    fn get_parsed_source_file(
        &self,
        opts: ast::SourceFileParseOptions,
    ) -> Option<ast::ParsedSourceFile> {
        let (text, ok) = self.fs().read_file(&opts.file_name);
        if !ok {
            return None;
        }
        let script_kind = core::get_script_kind_from_file_name(&opts.file_name);
        Some(parser::parse_source_file_as_parsed(opts, text, script_kind))
    }
    fn get_source_file(&self, opts: ast::SourceFileParseOptions) -> Option<ast::SourceFile> {
        self.get_parsed_source_file(opts)
            .map(ast::ParsedSourceFile::into_source_file)
    }
    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<tsoptions::ParsedCommandLine>;
}

pub struct CompilerHostImpl {
    current_directory: String,
    fs: Box<dyn vfs::Fs + Send + Sync>,
    default_library_path: String,
    extended_config_cache: Option<Box<dyn tsoptions::ExtendedConfigCache>>,
    trace: Box<Trace>,
    trace_text: Box<TraceText>,
}

pub fn new_cached_fs_compiler_host(
    current_directory: String,
    fs: Box<dyn vfs::Fs + Send + Sync>,
    default_library_path: String,
    extended_config_cache: Option<Box<dyn tsoptions::ExtendedConfigCache>>,
    trace: Option<Box<Trace>>,
) -> Box<dyn CompilerHost> {
    let fs: Arc<dyn vfs::Fs + Send + Sync> = fs.into();
    new_compiler_host_with_text_trace(
        current_directory,
        Box::new(vfs::cachedvfs::CachedFs::from(fs)),
        default_library_path,
        extended_config_cache,
        trace,
        None,
    )
}

pub fn new_compiler_host(
    current_directory: String,
    fs: Box<dyn vfs::Fs + Send + Sync>,
    default_library_path: String,
    extended_config_cache: Option<Box<dyn tsoptions::ExtendedConfigCache>>,
    trace: Option<Box<Trace>>,
) -> Box<dyn CompilerHost> {
    new_compiler_host_with_text_trace(
        current_directory,
        fs,
        default_library_path,
        extended_config_cache,
        trace,
        None,
    )
}

pub fn new_compiler_host_with_text_trace(
    current_directory: String,
    fs: Box<dyn vfs::Fs + Send + Sync>,
    default_library_path: String,
    extended_config_cache: Option<Box<dyn tsoptions::ExtendedConfigCache>>,
    trace: Option<Box<Trace>>,
    trace_text: Option<Box<TraceText>>,
) -> Box<dyn CompilerHost> {
    let trace = trace.unwrap_or_else(|| Box::new(|_msg, _args| {}));
    let trace_text = trace_text.unwrap_or_else(|| Box::new(|_msg| {}));
    Box::new(CompilerHostImpl {
        current_directory,
        fs,
        default_library_path,
        extended_config_cache,
        trace,
        trace_text,
    })
}

impl CompilerHost for CompilerHostImpl {
    fn fs(&self) -> &dyn vfs::Fs {
        self.fs.as_ref()
    }

    fn default_library_path(&self) -> String {
        self.default_library_path.clone()
    }

    fn get_current_directory(&self) -> String {
        self.current_directory.clone()
    }

    fn trace(&self, msg: &'static diagnostics::Message, args: &DiagnosticArgs) {
        (self.trace)(msg, args);
    }

    fn trace_text(&self, msg: &str) {
        (self.trace_text)(msg);
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<tsoptions::ParsedCommandLine> {
        let (command_line, _) = tsoptions::get_parsed_command_line_of_config_file_path(
            file_name,
            path,
            None,
            None,
            self,
            self.extended_config_cache
                .as_deref()
                .map(|cache| cache as &dyn tsoptions::ExtendedConfigCache),
        );
        command_line
    }
}

impl tsoptions::ParseConfigHost for CompilerHostImpl {
    fn fs(&self) -> &dyn vfs::Fs {
        CompilerHost::fs(self)
    }

    fn get_current_directory(&self) -> String {
        CompilerHost::get_current_directory(self)
    }
}
