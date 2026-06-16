use super::tracker::{SymbolAccessibility, SymbolAccessibilityResult};
use ts_ast as ast;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleNameDiagnostic {
    ExternalModuleCannotBeNamed,
    PrivateModule,
    PrivateName,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationDiagnosticContext {
    AccessorName,
    MethodName,
    VariableDeclarationType,
    AccessorDeclarationType,
    ReturnType,
    ParameterDeclarationType,
    TypeParameterConstraint,
    ExpressionWithTypeArguments,
    ImportEquals,
    TypeAlias,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IsolatedDeclarationDiagnostic {
    ExplicitReturnTypeRequired,
    ExplicitAccessorTypeRequired,
    ExplicitParameterTypeRequired,
    ExplicitVariableTypeRequired,
    ExplicitPropertyTypeRequired,
    ComputedPropertyCannotBeInferred,
    ObjectSpreadCannotBeInferred,
    ObjectShorthandCannotBeInferred,
    OnlyConstArraysCanBeInferred,
    DefaultExportCannotBeInferred,
    ArraySpreadCannotBeInferred,
    BindingElementCannotBeExportedDirectly,
    EntityInTypeNode,
    HeritageClauseExpression,
    ExpressionCannotBeInferred,
    ClassExpressionInferenceUnsupported,
    ImplicitUndefinedParameterUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelatedSuggestion {
    AddReturnTypeToFunctionExpression,
    AddReturnTypeToMethod,
    AddReturnTypeToGetAccessor,
    AddTypeToSetAccessorParameter,
    AddReturnTypeToFunctionDeclaration,
    AddTypeAnnotationToParameter,
    AddTypeAnnotationToVariable,
    AddTypeAnnotationToProperty,
    MoveDefaultExportExpressionToVariable,
    AddSatisfiesAndTypeAssertion,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeclarationNodeFacts {
    pub kind: ast::Kind,
    pub parent_kind: Option<ast::Kind>,
    pub grandparent_kind: Option<ast::Kind>,
    pub is_static: bool,
    pub is_parameter_property_of_private_constructor: bool,
    pub has_name: bool,
    pub has_initializer: bool,
    pub requires_implicit_undefined: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolAccessibilityDiagnostic {
    pub context: DeclarationDiagnosticContext,
    pub module_name_case: Option<ModuleNameDiagnostic>,
    pub error_node: &'static str,
    pub type_name_source: Option<&'static str>,
}

pub fn select_diagnostic_based_on_module_name(
    result: &SymbolAccessibilityResult,
) -> ModuleNameDiagnostic {
    if result
        .error_module_name
        .as_deref()
        .is_some_and(|name| !name.is_empty())
    {
        if result.accessibility == Some(SymbolAccessibility::CannotBeNamed) {
            ModuleNameDiagnostic::ExternalModuleCannotBeNamed
        } else {
            ModuleNameDiagnostic::PrivateModule
        }
    } else {
        ModuleNameDiagnostic::PrivateName
    }
}

pub fn select_diagnostic_based_on_module_name_no_name_check(
    result: &SymbolAccessibilityResult,
) -> ModuleNameDiagnostic {
    if result
        .error_module_name
        .as_deref()
        .is_some_and(|name| !name.is_empty())
    {
        ModuleNameDiagnostic::PrivateModule
    } else {
        ModuleNameDiagnostic::PrivateName
    }
}

pub fn symbol_accessibility_diagnostic_for_node_name(
    facts: DeclarationNodeFacts,
    result: &SymbolAccessibilityResult,
) -> Option<SymbolAccessibilityDiagnostic> {
    let context = match facts.kind {
        ast::Kind::SetAccessor | ast::Kind::GetAccessor => {
            DeclarationDiagnosticContext::AccessorName
        }
        ast::Kind::MethodDeclaration | ast::Kind::MethodSignature => {
            DeclarationDiagnosticContext::MethodName
        }
        _ => return symbol_accessibility_diagnostic_for_node(facts, result),
    };

    Some(SymbolAccessibilityDiagnostic {
        context,
        module_name_case: Some(accessor_or_method_name_module_case(facts, result)),
        error_node: "node",
        type_name_source: Some("declaration_name"),
    })
}

pub fn symbol_accessibility_diagnostic_for_node(
    facts: DeclarationNodeFacts,
    result: &SymbolAccessibilityResult,
) -> Option<SymbolAccessibilityDiagnostic> {
    let (context, error_node, type_name_source) = match facts.kind {
        ast::Kind::VariableDeclaration
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::PropertyAccessExpression
        | ast::Kind::ElementAccessExpression
        | ast::Kind::BinaryExpression
        | ast::Kind::BindingElement
        | ast::Kind::Constructor => (
            DeclarationDiagnosticContext::VariableDeclarationType,
            "node",
            Some("declaration_name"),
        ),
        ast::Kind::SetAccessor | ast::Kind::GetAccessor => (
            DeclarationDiagnosticContext::AccessorDeclarationType,
            "declaration_name",
            Some("declaration_name"),
        ),
        ast::Kind::ConstructSignature
        | ast::Kind::CallSignature
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::FunctionDeclaration
        | ast::Kind::IndexSignature => (
            DeclarationDiagnosticContext::ReturnType,
            "name_or_node",
            None,
        ),
        ast::Kind::Parameter => {
            if facts.is_parameter_property_of_private_constructor {
                (
                    DeclarationDiagnosticContext::VariableDeclarationType,
                    "node",
                    Some("declaration_name"),
                )
            } else {
                (
                    DeclarationDiagnosticContext::ParameterDeclarationType,
                    "node",
                    Some("declaration_name"),
                )
            }
        }
        ast::Kind::TypeParameter => (
            DeclarationDiagnosticContext::TypeParameterConstraint,
            "node",
            Some("declaration_name"),
        ),
        ast::Kind::ExpressionWithTypeArguments => (
            DeclarationDiagnosticContext::ExpressionWithTypeArguments,
            "node",
            Some("parent_declaration_name"),
        ),
        ast::Kind::ImportEqualsDeclaration => (
            DeclarationDiagnosticContext::ImportEquals,
            "node",
            Some("declaration_name"),
        ),
        ast::Kind::TypeAliasDeclaration | ast::Kind::JSTypeAliasDeclaration => (
            DeclarationDiagnosticContext::TypeAlias,
            "type_node",
            Some("declaration_name"),
        ),
        _ => return None,
    };

    Some(SymbolAccessibilityDiagnostic {
        context,
        module_name_case: module_case_for_context(context, facts, result),
        error_node,
        type_name_source,
    })
}

fn accessor_or_method_name_module_case(
    facts: DeclarationNodeFacts,
    result: &SymbolAccessibilityResult,
) -> ModuleNameDiagnostic {
    if facts.is_static || facts.parent_kind == Some(ast::Kind::ClassDeclaration) {
        select_diagnostic_based_on_module_name(result)
    } else {
        select_diagnostic_based_on_module_name_no_name_check(result)
    }
}

fn module_case_for_context(
    context: DeclarationDiagnosticContext,
    facts: DeclarationNodeFacts,
    result: &SymbolAccessibilityResult,
) -> Option<ModuleNameDiagnostic> {
    match context {
        DeclarationDiagnosticContext::ImportEquals
        | DeclarationDiagnosticContext::ExpressionWithTypeArguments
        | DeclarationDiagnosticContext::TypeParameterConstraint => None,
        DeclarationDiagnosticContext::TypeAlias => {
            Some(select_diagnostic_based_on_module_name_no_name_check(result))
        }
        DeclarationDiagnosticContext::AccessorDeclarationType => {
            if facts.kind == ast::Kind::SetAccessor {
                Some(select_diagnostic_based_on_module_name_no_name_check(result))
            } else {
                Some(select_diagnostic_based_on_module_name(result))
            }
        }
        DeclarationDiagnosticContext::VariableDeclarationType
        | DeclarationDiagnosticContext::AccessorName
        | DeclarationDiagnosticContext::MethodName
        | DeclarationDiagnosticContext::ReturnType
        | DeclarationDiagnosticContext::ParameterDeclarationType => {
            if facts.parent_kind == Some(ast::Kind::InterfaceDeclaration)
                || matches!(
                    facts.kind,
                    ast::Kind::ConstructSignature
                        | ast::Kind::CallSignature
                        | ast::Kind::IndexSignature
                )
            {
                Some(select_diagnostic_based_on_module_name_no_name_check(result))
            } else {
                Some(select_diagnostic_based_on_module_name(result))
            }
        }
    }
}

pub fn isolated_declaration_error_for_kind(
    kind: ast::Kind,
) -> Option<IsolatedDeclarationDiagnostic> {
    match kind {
        ast::Kind::FunctionExpression
        | ast::Kind::FunctionDeclaration
        | ast::Kind::ArrowFunction => {
            Some(IsolatedDeclarationDiagnostic::ExplicitReturnTypeRequired)
        }
        ast::Kind::MethodDeclaration | ast::Kind::ConstructSignature => {
            Some(IsolatedDeclarationDiagnostic::ExplicitReturnTypeRequired)
        }
        ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
            Some(IsolatedDeclarationDiagnostic::ExplicitAccessorTypeRequired)
        }
        ast::Kind::Parameter => Some(IsolatedDeclarationDiagnostic::ExplicitParameterTypeRequired),
        ast::Kind::VariableDeclaration => {
            Some(IsolatedDeclarationDiagnostic::ExplicitVariableTypeRequired)
        }
        ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => {
            Some(IsolatedDeclarationDiagnostic::ExplicitPropertyTypeRequired)
        }
        ast::Kind::ComputedPropertyName => {
            Some(IsolatedDeclarationDiagnostic::ComputedPropertyCannotBeInferred)
        }
        ast::Kind::SpreadAssignment => {
            Some(IsolatedDeclarationDiagnostic::ObjectSpreadCannotBeInferred)
        }
        ast::Kind::ShorthandPropertyAssignment => {
            Some(IsolatedDeclarationDiagnostic::ObjectShorthandCannotBeInferred)
        }
        ast::Kind::ArrayLiteralExpression => {
            Some(IsolatedDeclarationDiagnostic::OnlyConstArraysCanBeInferred)
        }
        ast::Kind::ExportAssignment => {
            Some(IsolatedDeclarationDiagnostic::DefaultExportCannotBeInferred)
        }
        ast::Kind::SpreadElement => {
            Some(IsolatedDeclarationDiagnostic::ArraySpreadCannotBeInferred)
        }
        _ => None,
    }
}

pub fn related_suggestion_for_declaration_kind(kind: ast::Kind) -> Option<RelatedSuggestion> {
    match kind {
        ast::Kind::ArrowFunction | ast::Kind::FunctionExpression => {
            Some(RelatedSuggestion::AddReturnTypeToFunctionExpression)
        }
        ast::Kind::MethodDeclaration => Some(RelatedSuggestion::AddReturnTypeToMethod),
        ast::Kind::GetAccessor => Some(RelatedSuggestion::AddReturnTypeToGetAccessor),
        ast::Kind::SetAccessor => Some(RelatedSuggestion::AddTypeToSetAccessorParameter),
        ast::Kind::FunctionDeclaration | ast::Kind::ConstructSignature => {
            Some(RelatedSuggestion::AddReturnTypeToFunctionDeclaration)
        }
        ast::Kind::Parameter => Some(RelatedSuggestion::AddTypeAnnotationToParameter),
        ast::Kind::VariableDeclaration => Some(RelatedSuggestion::AddTypeAnnotationToVariable),
        ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => {
            Some(RelatedSuggestion::AddTypeAnnotationToProperty)
        }
        ast::Kind::ExportAssignment => {
            Some(RelatedSuggestion::MoveDefaultExportExpressionToVariable)
        }
        _ => None,
    }
}

pub fn isolated_declaration_diagnostic_for_node(
    facts: DeclarationNodeFacts,
) -> IsolatedDeclarationDiagnostic {
    if facts.kind == ast::Kind::HeritageClause
        || facts.parent_kind == Some(ast::Kind::HeritageClause)
    {
        return IsolatedDeclarationDiagnostic::HeritageClauseExpression;
    }

    match facts.kind {
        ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
            IsolatedDeclarationDiagnostic::ExplicitAccessorTypeRequired
        }
        ast::Kind::ComputedPropertyName
        | ast::Kind::ShorthandPropertyAssignment
        | ast::Kind::SpreadAssignment => isolated_declaration_error_for_kind(facts.kind)
            .unwrap_or(IsolatedDeclarationDiagnostic::ExpressionCannotBeInferred),
        ast::Kind::ArrayLiteralExpression | ast::Kind::SpreadElement => {
            isolated_declaration_error_for_kind(facts.kind)
                .unwrap_or(IsolatedDeclarationDiagnostic::ExpressionCannotBeInferred)
        }
        ast::Kind::MethodDeclaration
        | ast::Kind::ConstructSignature
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction
        | ast::Kind::FunctionDeclaration => {
            IsolatedDeclarationDiagnostic::ExplicitReturnTypeRequired
        }
        ast::Kind::BindingElement => {
            IsolatedDeclarationDiagnostic::BindingElementCannotBeExportedDirectly
        }
        ast::Kind::PropertyDeclaration => {
            IsolatedDeclarationDiagnostic::ExplicitPropertyTypeRequired
        }
        ast::Kind::VariableDeclaration => {
            IsolatedDeclarationDiagnostic::ExplicitVariableTypeRequired
        }
        ast::Kind::Parameter if facts.requires_implicit_undefined => {
            IsolatedDeclarationDiagnostic::ImplicitUndefinedParameterUnsupported
        }
        ast::Kind::Parameter if facts.has_initializer => {
            IsolatedDeclarationDiagnostic::ExpressionCannotBeInferred
        }
        ast::Kind::Parameter => IsolatedDeclarationDiagnostic::ExplicitParameterTypeRequired,
        ast::Kind::ClassExpression => {
            IsolatedDeclarationDiagnostic::ClassExpressionInferenceUnsupported
        }
        _ => IsolatedDeclarationDiagnostic::ExpressionCannotBeInferred,
    }
}

pub fn is_declaration_enough_for_errors(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::ExportAssignment
            | ast::Kind::VariableStatement
            | ast::Kind::ExpressionStatement
            | ast::Kind::ReturnStatement
            | ast::Kind::VariableDeclaration
            | ast::Kind::PropertyDeclaration
            | ast::Kind::Parameter
    )
}

pub fn is_function_like_and_not_constructor(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::MethodDeclaration
    )
}
