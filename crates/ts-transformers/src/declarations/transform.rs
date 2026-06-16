#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationTransformAction {
    VisitSourceFile,
    VisitDeclarationStatements,
    ElideStatement,
    VisitExpressionStatement,
    VisitDeclarationSubtree,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopLevelDeclarationAction {
    TransformImportEquals,
    TransformImportDeclaration,
    TransformExportDeclaration,
    TransformExportAssignment,
    TransformTypeAlias,
    TransformInterface,
    TransformFunction,
    TransformModule,
    TransformClass,
    TransformVariableStatement,
    TransformEnum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationSubtreeAction {
    VisitChildren,
    TransformMappedType,
    TransformHeritageClause,
    TransformMethodSignature,
    TransformMethodDeclaration,
    TransformConstructSignature,
    TransformConstructor,
    TransformAccessor,
    TransformProperty,
    TransformCallOrIndexSignature,
    TransformVariableDeclaration,
    TransformTypeParameter,
    TransformExpressionWithTypeArguments,
    TransformTypeReference,
    TransformConditionalType,
    TransformFunctionType,
    TransformConstructorType,
    TransformImportType,
    TransformTypeQuery,
    TransformTupleType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpressionStatementAction {
    Elide,
    TransformCommonJsExport,
    TransformExpandoAssignment,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeclarationTransformFacts {
    pub is_declaration_file: bool,
    pub is_external_or_common_js_module: bool,
    pub result_has_external_module_indicator: bool,
    pub needs_scope_fix_marker: bool,
    pub result_has_scope_marker: bool,
    pub is_commonjs_export_assignment: bool,
    pub is_expando_assignment: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeclarationSourceFileState {
    pub needs_declare: bool,
    pub needs_scope_fix_marker: bool,
    pub result_has_scope_marker: bool,
    pub result_has_external_module_indicator: bool,
    pub suppress_new_diagnostic_contexts: bool,
    pub strip_export_modifiers: bool,
}

impl DeclarationSourceFileState {
    pub fn for_source_file() -> Self {
        Self {
            needs_declare: true,
            ..Self::default()
        }
    }
}

pub fn visit_source_file_should_transform(facts: DeclarationTransformFacts) -> bool {
    !facts.is_declaration_file
}

pub fn declaration_transform_action_for_kind(kind: ast::Kind) -> DeclarationTransformAction {
    match kind {
        ast::Kind::SourceFile => DeclarationTransformAction::VisitSourceFile,
        ast::Kind::FunctionDeclaration
        | ast::Kind::ModuleDeclaration
        | ast::Kind::ImportEqualsDeclaration
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::ClassDeclaration
        | ast::Kind::JSTypeAliasDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::VariableStatement
        | ast::Kind::ImportDeclaration
        | ast::Kind::JSImportDeclaration
        | ast::Kind::ExportDeclaration
        | ast::Kind::ExportAssignment => DeclarationTransformAction::VisitDeclarationStatements,
        ast::Kind::BreakStatement
        | ast::Kind::ContinueStatement
        | ast::Kind::DebuggerStatement
        | ast::Kind::DoStatement
        | ast::Kind::EmptyStatement
        | ast::Kind::ForInStatement
        | ast::Kind::ForOfStatement
        | ast::Kind::ForStatement
        | ast::Kind::IfStatement
        | ast::Kind::LabeledStatement
        | ast::Kind::ReturnStatement
        | ast::Kind::SwitchStatement
        | ast::Kind::ThrowStatement
        | ast::Kind::TryStatement
        | ast::Kind::WhileStatement
        | ast::Kind::WithStatement
        | ast::Kind::NotEmittedStatement
        | ast::Kind::Block
        | ast::Kind::MissingDeclaration => DeclarationTransformAction::ElideStatement,
        ast::Kind::ExpressionStatement => DeclarationTransformAction::VisitExpressionStatement,
        _ => DeclarationTransformAction::VisitDeclarationSubtree,
    }
}

pub fn top_level_declaration_action_for_kind(
    kind: ast::Kind,
    should_strip_internal: bool,
) -> Option<TopLevelDeclarationAction> {
    if should_strip_internal {
        return None;
    }

    Some(match kind {
        ast::Kind::ImportEqualsDeclaration => TopLevelDeclarationAction::TransformImportEquals,
        ast::Kind::ImportDeclaration | ast::Kind::JSImportDeclaration => {
            TopLevelDeclarationAction::TransformImportDeclaration
        }
        ast::Kind::ExportDeclaration => TopLevelDeclarationAction::TransformExportDeclaration,
        ast::Kind::ExportAssignment => TopLevelDeclarationAction::TransformExportAssignment,
        ast::Kind::TypeAliasDeclaration | ast::Kind::JSTypeAliasDeclaration => {
            TopLevelDeclarationAction::TransformTypeAlias
        }
        ast::Kind::InterfaceDeclaration => TopLevelDeclarationAction::TransformInterface,
        ast::Kind::FunctionDeclaration => TopLevelDeclarationAction::TransformFunction,
        ast::Kind::ModuleDeclaration => TopLevelDeclarationAction::TransformModule,
        ast::Kind::ClassDeclaration => TopLevelDeclarationAction::TransformClass,
        ast::Kind::VariableStatement => TopLevelDeclarationAction::TransformVariableStatement,
        ast::Kind::EnumDeclaration => TopLevelDeclarationAction::TransformEnum,
        _ => return None,
    })
}

pub fn declaration_subtree_action_for_kind(kind: ast::Kind) -> DeclarationSubtreeAction {
    match kind {
        ast::Kind::MappedType => DeclarationSubtreeAction::TransformMappedType,
        ast::Kind::HeritageClause => DeclarationSubtreeAction::TransformHeritageClause,
        ast::Kind::MethodSignature => DeclarationSubtreeAction::TransformMethodSignature,
        ast::Kind::MethodDeclaration => DeclarationSubtreeAction::TransformMethodDeclaration,
        ast::Kind::ConstructSignature => DeclarationSubtreeAction::TransformConstructSignature,
        ast::Kind::Constructor => DeclarationSubtreeAction::TransformConstructor,
        ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
            DeclarationSubtreeAction::TransformAccessor
        }
        ast::Kind::CallSignature | ast::Kind::IndexSignature => {
            DeclarationSubtreeAction::TransformCallOrIndexSignature
        }
        ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => {
            DeclarationSubtreeAction::TransformProperty
        }
        ast::Kind::VariableDeclaration => DeclarationSubtreeAction::TransformVariableDeclaration,
        ast::Kind::TypeParameter => DeclarationSubtreeAction::TransformTypeParameter,
        ast::Kind::ExpressionWithTypeArguments => {
            DeclarationSubtreeAction::TransformExpressionWithTypeArguments
        }
        ast::Kind::TypeReference => DeclarationSubtreeAction::TransformTypeReference,
        ast::Kind::ConditionalType => DeclarationSubtreeAction::TransformConditionalType,
        ast::Kind::FunctionType => DeclarationSubtreeAction::TransformFunctionType,
        ast::Kind::ConstructorType => DeclarationSubtreeAction::TransformConstructorType,
        ast::Kind::ImportType => DeclarationSubtreeAction::TransformImportType,
        ast::Kind::TypeQuery => DeclarationSubtreeAction::TransformTypeQuery,
        ast::Kind::TupleType => DeclarationSubtreeAction::TransformTupleType,
        _ => DeclarationSubtreeAction::VisitChildren,
    }
}

pub fn expression_statement_action(facts: DeclarationTransformFacts) -> ExpressionStatementAction {
    if facts.is_commonjs_export_assignment {
        ExpressionStatementAction::TransformCommonJsExport
    } else if facts.is_expando_assignment {
        ExpressionStatementAction::TransformExpandoAssignment
    } else {
        ExpressionStatementAction::Elide
    }
}

pub fn declaration_output_needs_empty_export_marker(facts: DeclarationTransformFacts) -> bool {
    facts.is_external_or_common_js_module
        && (!facts.result_has_external_module_indicator
            || (facts.needs_scope_fix_marker && !facts.result_has_scope_marker))
}
use ts_ast as ast;
