use ts_ast as ast;
use ts_core as core;
use ts_outputpaths as outputpaths;
use ts_tspath as tspath;

use crate::utilities::is_simple_copiable_expression_kind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationParentKind {
    EnumDeclaration,
    ModuleDeclaration,
    Other,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneratedIdentifierInfo {
    pub is_file_level: bool,
    pub is_optimistic: bool,
    pub is_reserved_in_nested_scopes: bool,
}

pub fn is_declaration_name_of_enum_or_namespace(
    parent_kind: DeclarationParentKind,
    is_parent_name: bool,
) -> bool {
    is_parent_name
        && matches!(
            parent_kind,
            DeclarationParentKind::EnumDeclaration | DeclarationParentKind::ModuleDeclaration
        )
}

pub fn rewrite_module_specifier_text(
    text: &str,
    compiler_options: &core::CompilerOptions,
) -> Option<String> {
    if !core::should_rewrite_module_specifier(text, compiler_options) {
        return None;
    }

    let jsx = if compiler_options.jsx == core::JsxEmit::Preserve {
        core::JsxEmit::Preserve
    } else {
        core::JsxEmit::None
    };
    let updated = tspath::change_extension(text, &outputpaths::get_output_extension(text, jsx));
    (updated != text).then_some(updated)
}

pub fn create_empty_imports_marker() -> &'static str {
    "export {}"
}

pub fn try_get_module_name_from_file(_file_name: Option<&str>) -> Option<String> {
    None
}

pub fn get_external_module_name_from_path(_file_name: &str, _reference_path: &str) -> String {
    String::new()
}

pub fn try_rename_external_module(_module_name: &str) -> Option<String> {
    None
}

pub fn is_file_level_reserved_generated_identifier(info: GeneratedIdentifierInfo) -> bool {
    info.is_file_level && info.is_optimistic && info.is_reserved_in_nested_scopes
}

pub fn is_simple_inlineable_expression(kind: ast::Kind, is_identifier: bool) -> bool {
    !is_identifier && is_simple_copiable_expression_kind(kind)
}
