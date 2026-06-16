use ts_ast as ast;
use ts_core::{ModuleKind, ScriptTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommonJsAction {
    Keep,
    VisitChildren,
    SkipSourceFile,
    TransformSourceFile,
    LowerSideEffectImport,
    LowerImportDeclaration,
    LowerImportEqualsToRequire,
    ElideLocalExportDeclaration,
    LowerNamedReExport,
    LowerNamespaceReExport,
    LowerExportStar,
    ElideExportEquals,
    LowerDefaultExportAssignment,
    StripExportFromFunction,
    StripExportAndAppendClassExports,
    LowerExportedVariableStatement,
    VisitTopLevelNestedVariableStatement,
    VisitTopLevelNestedForStatement,
    VisitTopLevelNestedForInOrOfStatement,
    VisitTopLevelNestedDoStatement,
    VisitTopLevelNestedWhileStatement,
    VisitTopLevelNestedLabeledStatement,
    VisitTopLevelNestedWithStatement,
    VisitTopLevelNestedIfStatement,
    VisitTopLevelNestedSwitchStatement,
    VisitTopLevelNestedCaseBlock,
    VisitTopLevelNestedCaseOrDefaultClause,
    VisitTopLevelNestedTryStatement,
    VisitTopLevelNestedCatchClause,
    VisitTopLevelNestedBlock,
    VisitForStatement,
    VisitForInOrOfStatement,
    VisitDiscardedValue,
    VisitParenthesizedExpression,
    VisitPartiallyEmittedExpression,
    RewriteAssignmentToExport,
    FlattenDestructuringAssignment,
    RewritePrefixUpdateToExport,
    RewritePostfixUpdateToExport,
    LowerDynamicImport,
    RewriteImportOrRequireCall,
    IndirectImportedCall,
    IndirectImportedTaggedTemplate,
    RewriteIdentifierReference,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommonJsFacts {
    pub is_declaration_file: bool,
    pub is_effective_external_module: bool,
    pub subtree_contains_dynamic_import: bool,
    pub subtree_contains_identifier: bool,
    pub has_import_clause: bool,
    pub is_external_module_import_equals: bool,
    pub has_module_specifier: bool,
    pub has_export_clause: bool,
    pub is_named_exports: bool,
    pub is_export_equals: bool,
    pub has_export_modifier: bool,
    pub has_default_modifier: bool,
    pub is_assignment_expression: bool,
    pub is_destructuring_assignment: bool,
    pub is_comma_expression: bool,
    pub is_update_operator: bool,
    pub is_identifier_operand: bool,
    pub is_identifier_expression: bool,
    pub is_import_call: bool,
    pub is_js_require_call: bool,
    pub has_call_arguments: bool,
    pub rewrite_relative_import_extensions: bool,
    pub should_transform_import_call: bool,
    pub module_kind: ModuleKind,
    pub language_version: ScriptTarget,
    pub exported_name_count: usize,
    pub is_local_name: bool,
    pub is_generated_identifier: bool,
    pub is_file_level_reserved_generated_identifier: bool,
    pub is_helper_name: bool,
    pub is_declaration_name_of_enum_or_namespace: bool,
}

pub fn common_js_action_for_top_level_kind(
    kind: ast::Kind,
    facts: CommonJsFacts,
) -> CommonJsAction {
    match kind {
        ast::Kind::ImportDeclaration if !facts.has_import_clause => {
            CommonJsAction::LowerSideEffectImport
        }
        ast::Kind::ImportDeclaration => CommonJsAction::LowerImportDeclaration,
        ast::Kind::ImportEqualsDeclaration if facts.is_external_module_import_equals => {
            CommonJsAction::LowerImportEqualsToRequire
        }
        ast::Kind::ExportDeclaration if !facts.has_module_specifier => {
            CommonJsAction::ElideLocalExportDeclaration
        }
        ast::Kind::ExportDeclaration if facts.has_export_clause && facts.is_named_exports => {
            CommonJsAction::LowerNamedReExport
        }
        ast::Kind::ExportDeclaration if facts.has_export_clause => {
            CommonJsAction::LowerNamespaceReExport
        }
        ast::Kind::ExportDeclaration => CommonJsAction::LowerExportStar,
        ast::Kind::ExportAssignment if facts.is_export_equals => CommonJsAction::ElideExportEquals,
        ast::Kind::ExportAssignment => CommonJsAction::LowerDefaultExportAssignment,
        ast::Kind::FunctionDeclaration if facts.has_export_modifier => {
            CommonJsAction::StripExportFromFunction
        }
        ast::Kind::ClassDeclaration if facts.has_export_modifier => {
            CommonJsAction::StripExportAndAppendClassExports
        }
        ast::Kind::VariableStatement if facts.has_export_modifier => {
            CommonJsAction::LowerExportedVariableStatement
        }
        ast::Kind::VariableStatement => CommonJsAction::VisitTopLevelNestedVariableStatement,
        ast::Kind::ForStatement => CommonJsAction::VisitTopLevelNestedForStatement,
        ast::Kind::ForInStatement | ast::Kind::ForOfStatement => {
            CommonJsAction::VisitTopLevelNestedForInOrOfStatement
        }
        ast::Kind::DoStatement => CommonJsAction::VisitTopLevelNestedDoStatement,
        ast::Kind::WhileStatement => CommonJsAction::VisitTopLevelNestedWhileStatement,
        ast::Kind::LabeledStatement => CommonJsAction::VisitTopLevelNestedLabeledStatement,
        ast::Kind::WithStatement => CommonJsAction::VisitTopLevelNestedWithStatement,
        ast::Kind::IfStatement => CommonJsAction::VisitTopLevelNestedIfStatement,
        ast::Kind::SwitchStatement => CommonJsAction::VisitTopLevelNestedSwitchStatement,
        ast::Kind::CaseBlock => CommonJsAction::VisitTopLevelNestedCaseBlock,
        ast::Kind::CaseClause | ast::Kind::DefaultClause => {
            CommonJsAction::VisitTopLevelNestedCaseOrDefaultClause
        }
        ast::Kind::TryStatement => CommonJsAction::VisitTopLevelNestedTryStatement,
        ast::Kind::CatchClause => CommonJsAction::VisitTopLevelNestedCatchClause,
        ast::Kind::Block => CommonJsAction::VisitTopLevelNestedBlock,
        _ => CommonJsAction::VisitChildren,
    }
}

pub fn common_js_action_for_kind(kind: ast::Kind, facts: CommonJsFacts) -> CommonJsAction {
    match kind {
        ast::Kind::SourceFile if facts.is_declaration_file => CommonJsAction::SkipSourceFile,
        ast::Kind::SourceFile
            if !(facts.is_effective_external_module || facts.subtree_contains_dynamic_import) =>
        {
            CommonJsAction::SkipSourceFile
        }
        ast::Kind::SourceFile => CommonJsAction::TransformSourceFile,
        _ if kind != ast::Kind::SourceFile
            && !(facts.subtree_contains_dynamic_import || facts.subtree_contains_identifier) =>
        {
            CommonJsAction::Keep
        }
        ast::Kind::ForStatement => CommonJsAction::VisitForStatement,
        ast::Kind::ForInStatement | ast::Kind::ForOfStatement => {
            CommonJsAction::VisitForInOrOfStatement
        }
        ast::Kind::ExpressionStatement | ast::Kind::VoidExpression => {
            CommonJsAction::VisitDiscardedValue
        }
        ast::Kind::ParenthesizedExpression => CommonJsAction::VisitParenthesizedExpression,
        ast::Kind::PartiallyEmittedExpression => CommonJsAction::VisitPartiallyEmittedExpression,
        ast::Kind::BinaryExpression if facts.is_destructuring_assignment => {
            CommonJsAction::FlattenDestructuringAssignment
        }
        ast::Kind::BinaryExpression
            if facts.is_assignment_expression && needs_export_update(facts) =>
        {
            CommonJsAction::RewriteAssignmentToExport
        }
        ast::Kind::BinaryExpression if facts.is_comma_expression => CommonJsAction::VisitChildren,
        ast::Kind::PrefixUnaryExpression
            if facts.is_update_operator && needs_export_update(facts) =>
        {
            CommonJsAction::RewritePrefixUpdateToExport
        }
        ast::Kind::PostfixUnaryExpression
            if facts.is_update_operator && needs_export_update(facts) =>
        {
            CommonJsAction::RewritePostfixUpdateToExport
        }
        ast::Kind::CallExpression
            if facts.is_import_call
                && facts.should_transform_import_call
                && should_lower_dynamic_import(facts.module_kind, facts.language_version) =>
        {
            CommonJsAction::LowerDynamicImport
        }
        ast::Kind::CallExpression
            if facts.rewrite_relative_import_extensions
                && facts.has_call_arguments
                && (facts.is_import_call || facts.is_js_require_call) =>
        {
            CommonJsAction::RewriteImportOrRequireCall
        }
        ast::Kind::CallExpression if facts.is_identifier_expression => {
            CommonJsAction::IndirectImportedCall
        }
        ast::Kind::TaggedTemplateExpression if facts.is_identifier_expression => {
            CommonJsAction::IndirectImportedTaggedTemplate
        }
        ast::Kind::Identifier if should_rewrite_identifier_reference(facts) => {
            CommonJsAction::RewriteIdentifierReference
        }
        _ => CommonJsAction::VisitChildren,
    }
}

pub fn needs_export_update(facts: CommonJsFacts) -> bool {
    facts.is_identifier_operand
        && facts.exported_name_count > 0
        && !facts.is_local_name
        && (!facts.is_generated_identifier || facts.is_file_level_reserved_generated_identifier)
}

pub fn should_rewrite_identifier_reference(facts: CommonJsFacts) -> bool {
    !facts.is_helper_name
        && !facts.is_local_name
        && !facts.is_declaration_name_of_enum_or_namespace
        && (!facts.is_generated_identifier || facts.is_file_level_reserved_generated_identifier)
}

pub fn should_transform_common_js_source_file(
    is_declaration_file: bool,
    is_effective_external_module: bool,
    subtree_contains_dynamic_import: bool,
) -> bool {
    !is_declaration_file && (is_effective_external_module || subtree_contains_dynamic_import)
}

pub fn should_emit_es_module_marker(
    is_supported_js_file: bool,
    has_common_js_module_indicator: bool,
    has_external_module_indicator: bool,
    external_module_indicator_is_source_file: bool,
    export_equals_present: bool,
) -> bool {
    if is_supported_js_file
        && has_common_js_module_indicator
        && (!has_external_module_indicator || external_module_indicator_is_source_file)
    {
        return false;
    }

    !export_equals_present && has_external_module_indicator
}

pub fn export_initialization_chunk_count(exported_name_count: usize) -> usize {
    exported_name_count.div_ceil(50)
}

pub fn should_lower_dynamic_import(
    module_kind: ModuleKind,
    language_version: ScriptTarget,
) -> bool {
    !(module_kind == ModuleKind::None && language_version >= ScriptTarget::ES2020)
}

pub fn dynamic_import_needs_sync_eval(argument_is_simple_inlineable: bool) -> bool {
    !argument_is_simple_inlineable
}
