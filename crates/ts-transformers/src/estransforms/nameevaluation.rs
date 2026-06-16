#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnonymousFunctionDefinitionKind {
    ClassExpression,
    FunctionExpression,
    ArrowFunction,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedEvaluationSourceKind {
    PropertyAssignment,
    ShorthandPropertyAssignment,
    VariableDeclaration,
    Parameter,
    BindingElement,
    PropertyDeclaration,
    BinaryExpression,
    ExportAssignment,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedEvaluationTransform {
    None,
    PropertyAssignment,
    ShorthandPropertyAssignment,
    VariableDeclaration,
    Parameter,
    BindingElement,
    PropertyDeclaration,
    AssignmentExpression,
    ExportAssignment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssignedName {
    Explicit(String),
    Identifier(String),
    PropertyName(String),
    Default,
    Empty,
    ComputedNeedsTemp,
}

pub fn is_anonymous_function_definition(
    kind: AnonymousFunctionDefinitionKind,
    class_has_declared_or_assigned_name: bool,
    function_has_name: bool,
) -> bool {
    match kind {
        AnonymousFunctionDefinitionKind::ClassExpression => !class_has_declared_or_assigned_name,
        AnonymousFunctionDefinitionKind::FunctionExpression => !function_has_name,
        AnonymousFunctionDefinitionKind::ArrowFunction => true,
        AnonymousFunctionDefinitionKind::Other => false,
    }
}

pub fn named_evaluation_transform_for_source(
    source: NamedEvaluationSourceKind,
    initializer_is_anonymous_function_definition: bool,
) -> NamedEvaluationTransform {
    if !initializer_is_anonymous_function_definition {
        return NamedEvaluationTransform::None;
    }

    match source {
        NamedEvaluationSourceKind::PropertyAssignment => {
            NamedEvaluationTransform::PropertyAssignment
        }
        NamedEvaluationSourceKind::ShorthandPropertyAssignment => {
            NamedEvaluationTransform::ShorthandPropertyAssignment
        }
        NamedEvaluationSourceKind::VariableDeclaration => {
            NamedEvaluationTransform::VariableDeclaration
        }
        NamedEvaluationSourceKind::Parameter => NamedEvaluationTransform::Parameter,
        NamedEvaluationSourceKind::BindingElement => NamedEvaluationTransform::BindingElement,
        NamedEvaluationSourceKind::PropertyDeclaration => {
            NamedEvaluationTransform::PropertyDeclaration
        }
        NamedEvaluationSourceKind::BinaryExpression => {
            NamedEvaluationTransform::AssignmentExpression
        }
        NamedEvaluationSourceKind::ExportAssignment => NamedEvaluationTransform::ExportAssignment,
        NamedEvaluationSourceKind::Other => NamedEvaluationTransform::None,
    }
}

pub fn assigned_name_of_identifier(
    explicit: Option<&str>,
    identifier_text: &str,
    original_is_anonymous_default_declaration: bool,
) -> AssignedName {
    if let Some(explicit) = explicit {
        return AssignedName::Explicit(explicit.to_owned());
    }
    if original_is_anonymous_default_declaration {
        AssignedName::Default
    } else {
        AssignedName::Identifier(identifier_text.to_owned())
    }
}

pub fn assigned_name_of_property_name(
    explicit: Option<&str>,
    literal_text: Option<&str>,
    computed_requires_temp: bool,
) -> AssignedName {
    if let Some(explicit) = explicit {
        return AssignedName::Explicit(explicit.to_owned());
    }
    if let Some(literal_text) = literal_text {
        return AssignedName::PropertyName(literal_text.to_owned());
    }
    if computed_requires_temp {
        AssignedName::ComputedNeedsTemp
    } else {
        AssignedName::Empty
    }
}

pub fn assigned_name_of_export_assignment(
    explicit: Option<&str>,
    is_export_equals: bool,
) -> AssignedName {
    if let Some(explicit) = explicit {
        AssignedName::Explicit(explicit.to_owned())
    } else if is_export_equals {
        AssignedName::Empty
    } else {
        AssignedName::Default
    }
}

pub fn should_finish_named_evaluation(
    ignore_empty_string_literal: bool,
    assigned_name_is_empty_string: bool,
) -> bool {
    !(ignore_empty_string_literal && assigned_name_is_empty_string)
}
