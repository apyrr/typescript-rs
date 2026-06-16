use std::ops::ControlFlow;

use ts_ast as ast;
use ts_core as core;
use ts_printer as printer;
use ts_scanner as scanner;

pub fn is_generated_identifier(
    emit_context: &mut printer::EmitContext,
    name: &ast::IdentifierNode,
) -> bool {
    emit_context.has_auto_generate_info(Some(name))
}

pub fn is_helper_name(emit_context: &mut printer::EmitContext, name: &ast::IdentifierNode) -> bool {
    emit_context.emit_flags(name) & printer::EF_HELPER_NAME != 0
}

pub fn is_local_name(emit_context: &mut printer::EmitContext, name: &ast::IdentifierNode) -> bool {
    emit_context.emit_flags(name) & printer::EF_LOCAL_NAME != 0
}

pub fn is_export_name(emit_context: &mut printer::EmitContext, name: &ast::IdentifierNode) -> bool {
    emit_context.emit_flags(name) & printer::EF_EXPORT_NAME != 0
}

// MoveRangePastModifiers returns a text range that starts past any modifiers on the node.
pub fn move_range_past_modifiers(store: &ast::AstStore, node: ast::Node) -> core::TextRange {
    if ast::is_property_declaration(store, node) || ast::is_method_declaration(store, node) {
        if let Some(name) = store.name(node) {
            return core::TextRange::new(store.loc(name).pos(), store.loc(node).end());
        }
    }
    let last_modifier = store
        .source_modifiers(node)
        .and_then(|modifiers| modifiers.nodes().iter().last());
    if let Some(last_modifier) = last_modifier {
        let modifier_loc = store.loc(last_modifier);
        if !ast::position_is_synthesized(modifier_loc.end()) {
            return core::TextRange::new(modifier_loc.end(), store.loc(node).end());
        }
    }
    move_range_past_decorators(store, node)
}

// MoveRangePastDecorators returns a text range that starts past any decorators on the node.
pub fn move_range_past_decorators(store: &ast::AstStore, node: ast::Node) -> core::TextRange {
    let last_decorator = store.source_modifiers(node).and_then(|modifiers| {
        modifiers.nodes().iter().fold(None, |last, modifier| {
            if store.kind(modifier) == ast::Kind::Decorator {
                Some(modifier)
            } else {
                last
            }
        })
    });
    if let Some(last_decorator) = last_decorator {
        let decorator_loc = store.loc(last_decorator);
        if !ast::position_is_synthesized(decorator_loc.end()) {
            return core::TextRange::new(decorator_loc.end(), store.loc(node).end());
        }
    }
    store.loc(node)
}

// Used in the module transformer to check if an expression is reasonably without side effect,
// and thus better to copy into multiple places rather than cache in a temporary variable.
pub fn is_simple_copiable_expression(source: &ast::AstStore, expression: &ast::Expression) -> bool {
    ast::is_string_literal_like(source, *expression)
        || ast::is_numeric_literal(source, *expression)
        || ast::is_keyword(source.kind(*expression))
        || ast::is_identifier(source, *expression)
}

pub fn is_identifier_reference(
    source: &ast::AstStore,
    name: &ast::IdentifierNode,
    parent: ast::Node,
) -> bool {
    fn is_same(left: Option<ast::Node>, right: ast::Node) -> bool {
        left.is_some_and(|left| left == right)
    }

    match source.kind(parent) {
        ast::Kind::BinaryExpression
        | ast::Kind::PrefixUnaryExpression
        | ast::Kind::PostfixUnaryExpression
        | ast::Kind::YieldExpression
        | ast::Kind::ElementAccessExpression
        | ast::Kind::NonNullExpression
        | ast::Kind::SpreadElement
        | ast::Kind::SpreadAssignment
        | ast::Kind::ParenthesizedExpression
        | ast::Kind::ArrayLiteralExpression
        | ast::Kind::DeleteExpression
        | ast::Kind::TypeOfExpression
        | ast::Kind::VoidExpression
        | ast::Kind::AwaitExpression
        | ast::Kind::JsxSelfClosingElement
        | ast::Kind::JsxSpreadAttribute
        | ast::Kind::JsxExpression
        | ast::Kind::PartiallyEmittedExpression => true,
        ast::Kind::ComputedPropertyName
        | ast::Kind::Decorator
        | ast::Kind::IfStatement
        | ast::Kind::DoStatement
        | ast::Kind::WhileStatement
        | ast::Kind::WithStatement
        | ast::Kind::ReturnStatement
        | ast::Kind::SwitchStatement
        | ast::Kind::CaseClause
        | ast::Kind::ThrowStatement
        | ast::Kind::ExpressionStatement
        | ast::Kind::ExportAssignment
        | ast::Kind::PropertyAccessExpression
        | ast::Kind::TemplateSpan => is_same(source.expression(parent), *name),
        ast::Kind::VariableDeclaration
        | ast::Kind::Parameter
        | ast::Kind::BindingElement
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::PropertyAssignment
        | ast::Kind::EnumMember
        | ast::Kind::JsxAttribute => is_same(source.initializer(parent), *name),
        ast::Kind::ForStatement => {
            is_same(source.initializer(parent), *name)
                || is_same(source.condition(parent), *name)
                || is_same(source.incrementor(parent), *name)
        }
        ast::Kind::ForInStatement | ast::Kind::ForOfStatement => {
            is_same(source.initializer(parent), *name) || is_same(source.expression(parent), *name)
        }
        ast::Kind::ImportEqualsDeclaration => is_same(source.module_reference(parent), *name),
        ast::Kind::ArrowFunction => is_same(source.body(parent), *name),
        ast::Kind::TypeAssertionExpression
        | ast::Kind::AsExpression
        | ast::Kind::SatisfiesExpression => is_same(source.expression(parent), *name),
        ast::Kind::ExpressionWithTypeArguments => {
            is_same(source.expression(parent), *name) && !ast::is_part_of_type_node(source, parent)
        }
        ast::Kind::ConditionalExpression => {
            is_same(source.condition(parent), *name)
                || is_same(source.when_true(parent), *name)
                || is_same(source.when_false(parent), *name)
        }
        ast::Kind::CallExpression => {
            is_same(source.expression(parent), *name)
                || source
                    .arguments(parent)
                    .expect("call expression should have arguments")
                    .iter()
                    .any(|arg| arg == *name)
        }
        ast::Kind::NewExpression => {
            is_same(source.expression(parent), *name)
                || source
                    .arguments(parent)
                    .is_some_and(|args| args.iter().any(|arg| arg == *name))
        }
        ast::Kind::TaggedTemplateExpression => is_same(source.tag(parent), *name),
        ast::Kind::ImportAttribute => is_same(source.value(parent), *name),
        ast::Kind::JsxOpeningElement => is_same(source.tag_name(parent), *name),
        ast::Kind::JsxClosingElement => is_same(source.tag_name(parent), *name),
        _ => false,
    }
}

pub fn single_or_many(
    nodes: Option<Vec<ast::Node>>,
    factory: &mut ast::NodeFactory,
) -> Option<ast::Node> {
    let nodes = nodes?;
    if nodes.len() == 1 {
        return nodes.into_iter().next();
    }
    Some(factory.new_syntax_list(nodes))
}

fn clone_node_for_emit_from_own_store(
    emit_context: &mut printer::EmitContext,
    node: ast::Node,
) -> ast::Node {
    if node.store_id() == emit_context.factory.node_factory.store().store_id() {
        return emit_context
            .factory
            .node_factory
            .deep_clone_node_in_current_store_preserve_location(node);
    }
    let source_file = emit_context
        .source_file_handle_for_node(node)
        .expect("emit context cannot resolve source node without a source file");
    let cloned = emit_context
        .factory
        .node_factory
        .deep_clone_node_from_store_preserve_location(source_file.store(), node);
    emit_context.set_original(&cloned, &node);
    cloned
}

fn convert_binding_element_to_array_assignment_element(
    emit_context: &mut printer::EmitContext,
    _source: &ast::AstStore,
    element: ast::Node,
) -> ast::Node {
    let (name, has_dot_dot_dot, initializer) = {
        let source = emit_context.store_for_node(element);
        (
            source.name(element),
            source.dot_dot_dot_token(element).is_some(),
            source.initializer(element),
        )
    };
    let Some(name) = name else {
        let omitted = emit_context.factory.node_factory.new_omitted_expression();
        emit_context.set_original(&omitted, &element);
        emit_context.assign_comment_and_source_map_ranges(&omitted, &element);
        return omitted;
    };
    if has_dot_dot_dot {
        let name = clone_node_for_emit_from_own_store(emit_context, name);
        let spread = emit_context.factory.node_factory.new_spread_element(name);
        emit_context.set_original(&spread, &element);
        emit_context.assign_comment_and_source_map_ranges(&spread, &element);
        return spread;
    }
    let expression = convert_binding_name_to_assignment_element_target(emit_context, _source, name);
    if let Some(initializer) = initializer {
        let initializer = clone_node_for_emit_from_own_store(emit_context, initializer);
        let assignment = emit_context
            .factory
            .new_assignment_expression(expression, initializer);
        emit_context.set_original(&assignment, &element);
        emit_context.assign_comment_and_source_map_ranges(&assignment, &element);
        return assignment;
    }
    expression
}

fn convert_binding_element_to_object_assignment_element(
    emit_context: &mut printer::EmitContext,
    _source: &ast::AstStore,
    element: ast::Node,
) -> ast::Node {
    let (has_dot_dot_dot, name, property_name, initializer) = {
        let source = emit_context.store_for_node(element);
        (
            source.dot_dot_dot_token(element).is_some(),
            source.name(element),
            source.property_name(element),
            source.initializer(element),
        )
    };
    if has_dot_dot_dot {
        let name = name.map(|name| clone_node_for_emit_from_own_store(emit_context, name));
        let spread = emit_context
            .factory
            .node_factory
            .new_spread_assignment(name);
        emit_context.set_original(&spread, &element);
        emit_context.assign_comment_and_source_map_ranges(&spread, &element);
        return spread;
    }
    if let Some(property_name) = property_name {
        let name = name.expect("binding element should have a name");
        let mut expression =
            convert_binding_name_to_assignment_element_target(emit_context, _source, name);
        if let Some(initializer) = initializer {
            let initializer = clone_node_for_emit_from_own_store(emit_context, initializer);
            expression = emit_context
                .factory
                .new_assignment_expression(expression, initializer);
        }
        let property_name = clone_node_for_emit_from_own_store(emit_context, property_name);
        let assignment = emit_context.factory.node_factory.new_property_assignment(
            None::<ast::ModifierList>,
            property_name,
            None::<ast::Node>,
            None::<ast::Node>,
            expression,
        );
        emit_context.set_original(&assignment, &element);
        emit_context.assign_comment_and_source_map_ranges(&assignment, &element);
        return assignment;
    }
    let equals_token = initializer.map(|_| {
        emit_context
            .factory
            .node_factory
            .new_token(ast::Kind::EqualsToken)
    });
    let name = name.map(|name| clone_node_for_emit_from_own_store(emit_context, name));
    let initializer = initializer
        .map(|initializer| clone_node_for_emit_from_own_store(emit_context, initializer));
    let assignment = emit_context
        .factory
        .node_factory
        .new_shorthand_property_assignment(
            None::<ast::ModifierList>,
            name,
            None::<ast::Node>,
            None::<ast::Node>,
            equals_token,
            initializer,
        );
    emit_context.set_original(&assignment, &element);
    emit_context.assign_comment_and_source_map_ranges(&assignment, &element);
    assignment
}

pub fn convert_binding_pattern_to_assignment_pattern(
    emit_context: &mut printer::EmitContext,
    _source: &ast::AstStore,
    element: ast::Node,
) -> ast::Node {
    let kind = emit_context.store_for_node(element).kind(element);
    match kind {
        ast::Kind::ArrayBindingPattern => {
            let (loc, range, element_nodes) = {
                let source = emit_context.store_for_node(element);
                let elements = source
                    .source_elements(element)
                    .expect("array binding pattern should have elements");
                (
                    elements.loc(),
                    elements.range(),
                    elements.iter().collect::<Vec<_>>(),
                )
            };
            let converted = element_nodes
                .iter()
                .map(|element| {
                    convert_binding_element_to_array_assignment_element(
                        emit_context,
                        _source,
                        *element,
                    )
                })
                .collect::<Vec<_>>();
            let element_list = emit_context
                .factory
                .node_factory
                .new_node_list(loc, range, converted);
            let object = emit_context
                .factory
                .node_factory
                .new_array_literal_expression(element_list, false);
            emit_context.set_original(&object, &element);
            emit_context.assign_comment_and_source_map_ranges(&object, &element);
            object
        }
        ast::Kind::ObjectBindingPattern => {
            let (loc, range, element_nodes) = {
                let source = emit_context.store_for_node(element);
                let elements = source
                    .source_elements(element)
                    .expect("object binding pattern should have elements");
                (
                    elements.loc(),
                    elements.range(),
                    elements.iter().collect::<Vec<_>>(),
                )
            };
            let converted = element_nodes
                .iter()
                .map(|element| {
                    convert_binding_element_to_object_assignment_element(
                        emit_context,
                        _source,
                        *element,
                    )
                })
                .collect::<Vec<_>>();
            let property_list = emit_context
                .factory
                .node_factory
                .new_node_list(loc, range, converted);
            let object = emit_context
                .factory
                .node_factory
                .new_object_literal_expression(property_list, false);
            emit_context.set_original(&object, &element);
            emit_context.assign_comment_and_source_map_ranges(&object, &element);
            object
        }
        _ => panic!("Unknown binding pattern"),
    }
}

fn convert_binding_name_to_assignment_element_target(
    emit_context: &mut printer::EmitContext,
    _source: &ast::AstStore,
    element: ast::Node,
) -> ast::Node {
    let is_binding_pattern = {
        let source = emit_context.store_for_node(element);
        ast::is_binding_pattern(source, element)
    };
    if is_binding_pattern {
        return convert_binding_pattern_to_assignment_pattern(emit_context, _source, element);
    }
    clone_node_for_emit_from_own_store(emit_context, element)
}

pub fn convert_variable_declaration_to_assignment_expression(
    emit_context: &mut printer::EmitContext,
    _source: &ast::AstStore,
    element: ast::Node,
) -> Option<ast::Node> {
    let (initializer, name) = {
        let source = emit_context.store_for_node(element);
        (
            source.initializer(element)?,
            source
                .name(element)
                .expect("variable declaration with initializer should have a name"),
        )
    };
    let expression = convert_binding_name_to_assignment_element_target(emit_context, _source, name);
    let initializer = clone_node_for_emit_from_own_store(emit_context, initializer);
    let assignment = emit_context
        .factory
        .new_assignment_expression(expression, initializer);
    emit_context.set_original(&assignment, &element);
    emit_context.assign_comment_and_source_map_ranges(&assignment, &element);
    Some(assignment)
}

pub fn copy_originals_for_preserved_subtree(
    emit_context: &mut printer::EmitContext,
    source: ast::Node,
    imported: ast::Node,
) {
    emit_context.set_original(&imported, &source);

    let source_children = collect_child_nodes(emit_context.store_for_node(source), source);
    let imported_children = collect_child_nodes(emit_context.store_for_node(imported), imported);
    for (source_child, imported_child) in source_children.into_iter().zip(imported_children) {
        copy_originals_for_preserved_subtree(emit_context, source_child, imported_child);
    }
}

fn collect_child_nodes(store: &ast::AstStore, node: ast::Node) -> Vec<ast::Node> {
    let mut children = Vec::new();
    let _ = store.for_each_child(node, |child| {
        if let Some(child) = child {
            children.push(child);
        }
        ControlFlow::Continue(())
    });
    children
}

pub fn is_simple_copiable_expression_kind(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::StringLiteral
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::Identifier
    ) || ast::is_keyword_kind(kind)
}

pub fn is_original_node_single_line(
    source: &ast::AstStore,
    emit_context: &printer::EmitContext,
    node: Option<ast::Node>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    let original = emit_context.most_original(&node);
    if original.store_id() != source.store_id() {
        return false;
    }
    let Some(source_file) = ast::get_source_file_of_node(source, Some(original)) else {
        return false;
    };
    let source_file = source.as_source_file(source_file);
    let loc = source.loc(original);
    let start_line = scanner::get_ecma_line_of_position(source_file, loc.pos().max(0) as usize);
    let end_line = scanner::get_ecma_line_of_position(source_file, loc.end().max(0) as usize);
    start_line == end_line
}
