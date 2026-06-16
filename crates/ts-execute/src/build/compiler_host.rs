use std::sync::Arc;
use ts_compiler as compiler;
use ts_diagnostics as diagnostics;
use ts_tspath as tspath;
use ts_vfs as vfs;

pub type Path = tspath::Path;
pub type DiagnosticsMessage = diagnostics::Message;
pub type DiagnosticArgs = [diagnostics::Any];

use super::host::Host;

pub struct CompilerHost {
    pub host: Arc<Host<Box<dyn compiler::CompilerHost>>>,
    pub trace: Box<dyn Fn(&DiagnosticsMessage, Vec<serde_json::Value>) + Send + Sync>,
}

impl compiler::CompilerHost for CompilerHost {
    fn fs(&self) -> &dyn vfs::Fs {
        self.host.fs()
    }

    fn default_library_path(&self) -> String {
        self.host.default_library_path()
    }

    fn get_current_directory(&self) -> String {
        self.host.get_current_directory()
    }

    fn trace(&self, msg: &'static DiagnosticsMessage, args: &DiagnosticArgs) {
        (self.trace)(
            msg,
            args.iter()
                .map(|arg| serde_json::Value::String(arg.to_string()))
                .collect(),
        );
    }

    fn trace_text(&self, msg: &str) {
        let message = diagnostics::Message::new(
            0,
            diagnostics::Category::Message,
            String::new(),
            msg.to_owned(),
        );
        (self.trace)(&message, Vec::new());
    }

    fn get_source_file(&self, opts: ts_ast::SourceFileParseOptions) -> Option<ts_ast::SourceFile> {
        self.host.get_source_file(opts)
    }

    fn get_parsed_source_file(
        &self,
        opts: ts_ast::SourceFileParseOptions,
    ) -> Option<ts_ast::ParsedSourceFile> {
        self.host.get_parsed_source_file(opts)
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: Path,
    ) -> Option<ts_tsoptions::ParsedCommandLine> {
        self.host.get_resolved_project_reference(file_name, path)
    }
}
