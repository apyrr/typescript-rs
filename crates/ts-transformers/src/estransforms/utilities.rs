use ts_ast as ast;
use ts_printer::{AutoGenerateOptions, EmitContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotNullConditionOperator {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EqualityOperator {
    StrictNotEquals,
    StrictEquals,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotNullConditionShape {
    pub equality: EqualityOperator,
    pub combine_with: NotNullConditionOperator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuperAccessAction {
    VisitChildren,
    StopAtNestedScope,
    SubstituteSuperPropertyAccess,
    SubstituteSuperElementAccess,
    SubstituteSuperCall,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SuperAccessState {
    pub captured_super_properties: Vec<String>,
    pub has_super_element_access: bool,
    pub has_super_property_assignment: bool,
}

pub fn not_null_condition_shape(invert: bool) -> NotNullConditionShape {
    if invert {
        NotNullConditionShape {
            equality: EqualityOperator::StrictEquals,
            combine_with: NotNullConditionOperator::Or,
        }
    } else {
        NotNullConditionShape {
            equality: EqualityOperator::StrictNotEquals,
            combine_with: NotNullConditionOperator::And,
        }
    }
}

pub fn super_access_action_for_kind(
    kind: ast::Kind,
    is_super_property_or_element_call: bool,
    expression_is_super: bool,
) -> SuperAccessAction {
    match kind {
        ast::Kind::CallExpression if is_super_property_or_element_call => {
            SuperAccessAction::SubstituteSuperCall
        }
        ast::Kind::CallExpression => SuperAccessAction::VisitChildren,
        ast::Kind::PropertyAccessExpression if expression_is_super => {
            SuperAccessAction::SubstituteSuperPropertyAccess
        }
        ast::Kind::ElementAccessExpression if expression_is_super => {
            SuperAccessAction::SubstituteSuperElementAccess
        }
        ast::Kind::FunctionExpression
        | ast::Kind::FunctionDeclaration
        | ast::Kind::MethodDeclaration
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::Constructor
        | ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression => SuperAccessAction::StopAtNestedScope,
        _ => SuperAccessAction::VisitChildren,
    }
}

pub fn track_super_access(
    state: &mut SuperAccessState,
    kind: ast::Kind,
    expression_is_super: bool,
    property_name: Option<&str>,
    assignment_target_contains_super_property: bool,
    is_update_expression: bool,
) {
    match kind {
        ast::Kind::PropertyAccessExpression if expression_is_super => {
            if let Some(property_name) = property_name
                && !state
                    .captured_super_properties
                    .iter()
                    .any(|captured| captured == property_name)
            {
                state
                    .captured_super_properties
                    .push(property_name.to_owned());
            }
        }
        ast::Kind::ElementAccessExpression if expression_is_super => {
            state.has_super_element_access = true;
        }
        ast::Kind::BinaryExpression if assignment_target_contains_super_property => {
            state.has_super_property_assignment = true;
        }
        ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression
            if is_update_expression && assignment_target_contains_super_property =>
        {
            state.has_super_property_assignment = true;
        }
        _ => {}
    }
}

pub fn super_element_access_reads_value_property(has_super_property_assignment: bool) -> bool {
    has_super_property_assignment
}

pub fn convert_class_declaration_removes_export_default() -> bool {
    true
}

pub fn accessor_backing_field_suffix() -> &'static str {
    "_accessor_storage"
}

// createAccessorPropertyBackingField creates a private backing field for an `accessor` PropertyDeclaration.
pub fn create_accessor_property_backing_field(
    emit_context: &mut EmitContext,
    source: &ast::AstStore,
    node: ast::Node,
    modifiers: impl ast::IntoOptionalModifierList,
    initializer: impl Into<Option<ast::Node>>,
) -> ast::Node {
    let options = AutoGenerateOptions {
        suffix: accessor_backing_field_suffix(),
        ..Default::default()
    };
    if node.store_id() == emit_context.factory.node_factory.store().store_id() {
        let name = emit_context
            .factory
            .node_factory
            .store()
            .name(node)
            .expect("auto-accessor should have a property name");
        let backing_field_name = emit_context
            .factory
            .new_generated_private_name_for_factory_node_ex(&name, options);
        return emit_context
            .factory
            .node_factory
            .update_property_declaration(
                node,
                modifiers,
                Some(backing_field_name),
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            );
    }
    assert_eq!(
        node.store_id(),
        source.store_id(),
        "auto-accessor backing field cannot read unrelated AST store"
    );
    let name = source
        .name(node)
        .expect("auto-accessor should have a property name");
    let backing_field_name = emit_context
        .factory
        .new_generated_private_name_for_node_ex(source, &name, options);
    emit_context
        .factory
        .node_factory
        .update_property_declaration_from_store(
            source,
            node,
            modifiers,
            Some(backing_field_name),
            None::<ast::Node>,
            None::<ast::Node>,
            initializer,
        )
}

pub fn es_transform_utility_name(name: &str) -> String {
    name.to_owned()
}
