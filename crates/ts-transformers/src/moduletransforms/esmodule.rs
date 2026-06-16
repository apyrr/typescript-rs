use ts_ast as ast;
use ts_core::ModuleKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EsModuleAction {
    Keep,
    VisitChildren,
    SkipSourceFile,
    TransformSourceFile,
    RewriteImportDeclaration,
    ElideImportEquals,
    LowerImportEqualsToRequire,
    PanicImportEqualsInternalModuleReference,
    VisitExportAssignment,
    ElideExportEquals,
    RewriteExportEqualsToModuleExports,
    RewriteExportDeclaration,
    SplitNamespaceReExport,
    RewriteImportOrRequireCall,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EsModuleFacts {
    pub is_declaration_file: bool,
    pub is_external_module: bool,
    pub isolated_modules: bool,
    pub rewrite_relative_import_extensions: bool,
    pub emit_module_kind: ModuleKind,
    pub configured_module_kind: ModuleKind,
    pub has_module_specifier: bool,
    pub has_export_clause: bool,
    pub is_namespace_export: bool,
    pub is_export_namespace_as_default: bool,
    pub is_export_equals: bool,
    pub is_external_module_import_equals: bool,
    pub is_import_call: bool,
    pub is_js_require_call: bool,
    pub has_call_arguments: bool,
}

pub fn es_module_action_for_kind(kind: ast::Kind, facts: EsModuleFacts) -> EsModuleAction {
    match kind {
        ast::Kind::SourceFile if facts.is_declaration_file => EsModuleAction::SkipSourceFile,
        ast::Kind::SourceFile if !(facts.is_external_module || facts.isolated_modules) => {
            EsModuleAction::SkipSourceFile
        }
        ast::Kind::SourceFile => EsModuleAction::TransformSourceFile,
        ast::Kind::ImportDeclaration if facts.rewrite_relative_import_extensions => {
            EsModuleAction::RewriteImportDeclaration
        }
        ast::Kind::ImportDeclaration => EsModuleAction::Keep,
        ast::Kind::ImportEqualsDeclaration if facts.emit_module_kind < ModuleKind::Node16 => {
            EsModuleAction::ElideImportEquals
        }
        ast::Kind::ImportEqualsDeclaration if facts.is_external_module_import_equals => {
            EsModuleAction::LowerImportEqualsToRequire
        }
        ast::Kind::ImportEqualsDeclaration => {
            EsModuleAction::PanicImportEqualsInternalModuleReference
        }
        ast::Kind::ExportAssignment if !facts.is_export_equals => {
            EsModuleAction::VisitExportAssignment
        }
        ast::Kind::ExportAssignment if facts.emit_module_kind != ModuleKind::Preserve => {
            EsModuleAction::ElideExportEquals
        }
        ast::Kind::ExportAssignment => EsModuleAction::RewriteExportEqualsToModuleExports,
        ast::Kind::ExportDeclaration if !facts.has_module_specifier => EsModuleAction::Keep,
        ast::Kind::ExportDeclaration
            if facts.configured_module_kind <= ModuleKind::ES2015
                && facts.has_export_clause
                && facts.is_namespace_export =>
        {
            EsModuleAction::SplitNamespaceReExport
        }
        ast::Kind::ExportDeclaration => EsModuleAction::RewriteExportDeclaration,
        ast::Kind::CallExpression
            if facts.rewrite_relative_import_extensions
                && facts.has_call_arguments
                && (facts.is_import_call || facts.is_js_require_call) =>
        {
            EsModuleAction::RewriteImportOrRequireCall
        }
        ast::Kind::CallExpression => EsModuleAction::VisitChildren,
        _ => EsModuleAction::VisitChildren,
    }
}

pub fn needs_empty_imports_marker(
    is_external_module: bool,
    emit_module_kind: ModuleKind,
    has_external_module_indicator: bool,
) -> bool {
    is_external_module && emit_module_kind != ModuleKind::Preserve && !has_external_module_indicator
}

pub fn create_require_uses_plain_require(emit_module_kind: ModuleKind) -> bool {
    emit_module_kind == ModuleKind::Preserve
}

pub fn namespace_reexport_outputs_default_assignment(is_export_namespace_as_default: bool) -> bool {
    is_export_namespace_as_default
}
