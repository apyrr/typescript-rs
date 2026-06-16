use std::sync::Arc;
use ts_ast as ast;
use ts_binder as binder;
use ts_core as core;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::namegenerator::LocalNameBindingFacts;

pub trait EmitBindingFacts: LocalNameBindingFacts {
    fn root(&self) -> ast::Node;
    fn source_symbol(&self) -> Option<ast::SymbolHandle>;
    fn symbol(&self, node: ast::Node) -> Option<ast::SymbolHandle>;
}

impl LocalNameBindingFacts for binder::ProgramBindingState {
    fn symbol_flags(&self, symbol: ast::SymbolHandle) -> ast::SymbolFlags {
        self.symbol_flags(symbol)
    }

    fn lookup_local(&self, node: ast::Node, name: &str) -> Option<ast::SymbolHandle> {
        self.lookup_local(node, name)
    }

    fn next_container(&self, node: ast::Node) -> Option<ast::Node> {
        self.next_container(node)
    }
}

impl EmitBindingFacts for binder::ProgramBindingState {
    fn root(&self) -> ast::Node {
        self.root()
    }

    fn source_symbol(&self) -> Option<ast::SymbolHandle> {
        self.source_symbol()
    }

    fn symbol(&self, node: ast::Node) -> Option<ast::SymbolHandle> {
        self.symbol(node)
    }
}

// NOTE: EmitHost operations must be thread-safe
pub trait EmitHost {
    fn options(&self) -> Option<core::CompilerOptions>;
    fn source_files(&self) -> Vec<ast::SourceFile>;
    fn use_case_sensitive_file_names(&self) -> bool;
    fn get_current_directory(&self) -> String;
    fn common_source_directory(&self) -> String;
    fn is_emit_blocked(&self, file: &str) -> bool;
    fn write_file(&self, file_name: &str, text: &str) -> Result<(), String>;
    fn emit_binding_facts(&self, file: &ast::SourceFile) -> Arc<dyn EmitBindingFacts>;
    fn source_file_common_js_module_indicator(&self, file: &ast::SourceFile) -> Option<ast::Node>;
    fn source_file_external_module_indicator(&self, file: &ast::SourceFile) -> Option<ast::Node>;
    fn source_file_export_equals_declarations(&self, file: &ast::SourceFile) -> Vec<ast::Node>;
    fn source_file_nested_cjs_exports(&self, file: &ast::SourceFile) -> Vec<ast::Node>;
    fn get_emit_module_format_of_file(&self, file: &dyn ast::HasFileName) -> core::ModuleKind;
    fn can_include_bind_and_check_diagnostics(&self, file: &ast::SourceFile) -> bool;
    fn with_emit_resolver(&mut self, f: &mut dyn FnMut(&mut dyn crate::EmitResolver));
    fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference>;
    fn get_source_file_from_reference(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile>;
    fn is_source_file_from_external_library(&self, file: &ast::SourceFile) -> bool;
}

pub fn with_emit_resolver<R>(
    host: &mut dyn EmitHost,
    mut f: impl FnMut(&mut dyn crate::EmitResolver) -> R,
) -> R {
    let mut result = None;
    host.with_emit_resolver(&mut |resolver| {
        result = Some(f(resolver));
    });
    result.expect("emit resolver callback must run")
}
