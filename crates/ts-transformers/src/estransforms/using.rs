use std::collections::{BTreeMap, BTreeSet};

use ts_ast::{self as ast, AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer::{self as printer, AutoGenerateOptions, GeneratedIdentifierFlags};

use super::classthis;
use crate::utilities;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum UsingKind {
    #[default]
    None,
    Sync,
    Async,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsingAction {
    Keep,
    VisitChildren,
    SkipSourceFile,
    TransformSourceFile,
    TransformBlock,
    ShallowRewriteForStatement,
    ShallowRewriteForOfStatement,
    DownlevelUsingDeclaration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoistAction {
    KeepInBody,
    HoistToTopLevel,
    HoistExportDefault,
    HoistExportEquals,
    HoistClassDeclaration,
    HoistVariableStatement,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UsingHoistState {
    pub export_bindings: BTreeMap<String, String>,
    pub export_vars: BTreeSet<String>,
    pub default_export_binding: Option<String>,
    pub export_equals_binding: Option<String>,
}

pub fn using_action_for_kind(
    kind: ast::Kind,
    subtree_contains_using: bool,
    is_declaration_file: bool,
    using_kind: UsingKind,
    initializer_is_using: bool,
) -> UsingAction {
    if !subtree_contains_using {
        return UsingAction::Keep;
    }

    match kind {
        ast::Kind::SourceFile if is_declaration_file => UsingAction::SkipSourceFile,
        ast::Kind::SourceFile if using_kind != UsingKind::None => UsingAction::TransformSourceFile,
        ast::Kind::SourceFile => UsingAction::VisitChildren,
        ast::Kind::Block if using_kind != UsingKind::None => UsingAction::TransformBlock,
        ast::Kind::ForStatement if initializer_is_using => UsingAction::ShallowRewriteForStatement,
        ast::Kind::ForOfStatement if initializer_is_using => {
            UsingAction::ShallowRewriteForOfStatement
        }
        _ => UsingAction::VisitChildren,
    }
}

pub fn using_kind_from_flags(is_using: bool, is_await_using: bool) -> UsingKind {
    if is_await_using {
        UsingKind::Async
    } else if is_using {
        UsingKind::Sync
    } else {
        UsingKind::None
    }
}

pub fn using_kind_of_statements<I>(kinds: I) -> UsingKind
where
    I: IntoIterator<Item = UsingKind>,
{
    let mut result = UsingKind::None;
    for kind in kinds {
        if kind == UsingKind::Async {
            return UsingKind::Async;
        }
        if kind > result {
            result = kind;
        }
    }
    result
}

pub fn hoist_action_for_top_level_body_statement(
    kind: ast::Kind,
    is_export_equals: bool,
) -> HoistAction {
    match kind {
        ast::Kind::ImportDeclaration
        | ast::Kind::ImportEqualsDeclaration
        | ast::Kind::ExportDeclaration
        | ast::Kind::FunctionDeclaration => HoistAction::HoistToTopLevel,
        ast::Kind::ExportAssignment if is_export_equals => HoistAction::HoistExportEquals,
        ast::Kind::ExportAssignment => HoistAction::HoistExportDefault,
        ast::Kind::ClassDeclaration => HoistAction::HoistClassDeclaration,
        ast::Kind::VariableStatement => HoistAction::HoistVariableStatement,
        _ => HoistAction::KeepInBody,
    }
}

pub fn add_disposable_resource_is_async(kind: UsingKind) -> bool {
    kind == UsingKind::Async
}

pub fn downlevel_disposal_needs_await(kind: UsingKind) -> bool {
    kind == UsingKind::Async
}

pub fn using_declaration_is_valid_binding(is_identifier_name: bool) -> bool {
    is_identifier_name
}

pub fn env_object_property_names() -> [&'static str; 3] {
    ["stack", "error", "hasError"]
}

pub fn default_export_temp_name() -> &'static str {
    "_default"
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    let mut runtime = UsingDeclarationTransformer {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        export_bindings: BTreeMap::new(),
        export_vars: Vec::new(),
        default_export_binding: None,
        export_equals_binding: None,
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

struct UsingDeclarationTransformer<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    export_bindings: BTreeMap<String, ast::Node>,
    export_vars: Vec<ast::Node>,
    default_export_binding: Option<ast::Node>,
    export_equals_binding: Option<ast::Node>,
}

impl UsingDeclarationTransformer<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    // Indicates whether an expression is an anonymous function definition.
    //
    // See https://tc39.es/ecma262/#sec-isanonymousfunctiondefinition
    fn is_anonymous_function_definition(&mut self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        let inner = ast::skip_outer_expressions(source, node, ast::OuterExpressionKinds::ALL);
        let inner_source = self.store_for(inner);
        match inner_source.kind(inner) {
            ast::Kind::ClassExpression => {
                !self.class_has_declared_or_explicitly_assigned_name(inner)
            }
            ast::Kind::FunctionExpression => inner_source.name(inner).is_none(),
            ast::Kind::ArrowFunction => true,
            _ => false,
        }
    }

    fn is_named_evaluation(&mut self, node: ast::Node) -> bool {
        if !ast::is_named_evaluation_source(self.store_for(node), node) {
            return false;
        }
        let source = self.store_for(node);
        let expression = match source.kind(node) {
            ast::Kind::ShorthandPropertyAssignment => source.object_assignment_initializer(node),
            ast::Kind::PropertyAssignment
            | ast::Kind::VariableDeclaration
            | ast::Kind::Parameter
            | ast::Kind::BindingElement
            | ast::Kind::PropertyDeclaration => source.initializer(node),
            ast::Kind::BinaryExpression => source.right(node),
            ast::Kind::ExportAssignment => source.expression(node),
            _ => None,
        };
        expression.is_some_and(|expression| self.is_anonymous_function_definition(expression))
    }

    // Gets whether a `ClassLikeDeclaration` has a `static {}` block containing only a single call to the
    // `__setFunctionName` helper.
    fn class_has_explicitly_assigned_name(&mut self, node: ast::Node) -> bool {
        if self.emit_context.assigned_name(&node).is_none() {
            return false;
        }
        let Some(member_nodes) = self
            .store_for(node)
            .members(node)
            .map(|members| members.iter().collect::<Vec<_>>())
        else {
            return false;
        };
        member_nodes
            .into_iter()
            .any(|member| self.is_class_named_evaluation_helper_block(member))
    }

    // Gets whether a `ClassLikeDeclaration` has a declared name or contains a `static {}` block containing only a single
    // call to the `__setFunctionName` helper.
    fn class_has_declared_or_explicitly_assigned_name(&mut self, node: ast::Node) -> bool {
        self.store_for(node).name(node).is_some() || self.class_has_explicitly_assigned_name(node)
    }

    // Gets whether a node is a `static {}` block containing only a single call to the `__setFunctionName` helper where that
    // call's second argument is the value stored in the `assignedName` property of the block's `EmitNode`.
    fn is_class_named_evaluation_helper_block(&mut self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        if !ast::is_class_static_block_declaration(source, node) {
            return false;
        }
        let Some(assigned_name) = self.emit_context.assigned_name(&node) else {
            return false;
        };
        let Some(body) = source.body(node) else {
            return false;
        };
        let Some(statements) = self.store_for(body).statements(body) else {
            return false;
        };
        if statements.len() != 1 {
            return false;
        }
        let Some(statement) = statements.first() else {
            return false;
        };
        let statement_source = self.store_for(statement);
        if statement_source.kind(statement) != ast::Kind::ExpressionStatement {
            return false;
        }
        let Some(expression) = statement_source.expression(statement) else {
            return false;
        };
        if !self
            .emit_context
            .is_call_to_helper(&expression, "__setFunctionName")
        {
            return false;
        }
        let Some(arguments) = self.store_for(expression).arguments(expression) else {
            return false;
        };
        arguments.len() >= 2 && arguments.iter().nth(1) == Some(assigned_name)
    }

    // Gets whether a node is a `static {}` block containing only a single assignment of the static `this` to the `_classThis`
    // (or similar) variable stored in the `classthis` property of the block's `EmitNode`.
    fn is_class_this_assignment_block(&mut self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        let Some(body) = source.body(node) else {
            return false;
        };
        let Some(statements) = self.store_for(body).statements(body) else {
            return false;
        };
        let Some(statement) = statements.first() else {
            return false;
        };
        let statement_source = self.store_for(statement);
        let expression = statement_source.expression(statement);
        let expression_source = expression.map(|expression| self.store_for(expression));
        let left = expression.and_then(|expression| expression_source?.left(expression));
        let right = expression.and_then(|expression| expression_source?.right(expression));
        let class_this = self.emit_context.class_this(&node);

        classthis::is_class_this_assignment_block_shape(
            ast::is_class_static_block_declaration(source, node),
            statements.len(),
            statement_source.kind(statement) == ast::Kind::ExpressionStatement,
            expression.is_some_and(|expression| {
                ast::is_assignment_expression(self.store_for(expression), expression, true)
            }),
            left.is_some_and(|left| ast::is_identifier(self.store_for(left), left)),
            left.zip(class_this)
                .is_some_and(|(left, class_this)| left == class_this),
            right.is_some_and(|right| self.store_for(right).kind(right) == ast::Kind::ThisKeyword),
        )
    }

    fn create_class_named_evaluation_helper_block(
        &mut self,
        assigned_name: ast::Node,
    ) -> ast::Node {
        // The assignedName parameter is the expression used to resolve the assigned name at runtime. This expression should not produce
        // side effects.
        // produces:
        //
        //  static { __setFunctionName(this, "C"); }
        //
        let this_expression = self.emit_context.factory.new_this_expression();
        let expression = self.emit_context.factory.new_set_function_name_helper(
            this_expression,
            assigned_name,
            "",
        );
        let statement = self.factory_mut().new_expression_statement(expression);
        let statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let body = self.factory_mut().new_block(statements, false);
        let block = self
            .factory_mut()
            .new_class_static_block_declaration(None::<ast::ModifierList>, Some(body));

        // We use `emitNode.assignedName` to indicate this is a NamedEvaluation helper block
        // and to stash the expression used to resolve the assigned name.
        self.emit_context.set_assigned_name(&block, &assigned_name);
        block
    }

    // Injects a class `static {}` block used to dynamically set the name of a class, if one does not already exist.
    fn inject_class_named_evaluation_helper_block_if_missing(
        &mut self,
        node: ast::Node,
        assigned_name: ast::Node,
    ) -> ast::Node {
        // given:
        //
        //  let C = class {
        //  };
        //
        // produces:
        //
        //  let C = class {
        //      static { __setFunctionName(this, "C"); }
        //  };
        //
        // NOTE: If the class has a `_classThis` assignment block, this helper will be injected after that block.
        if self.class_has_explicitly_assigned_name(node) {
            return self.preserve_node(node);
        }

        let (
            modifiers,
            name,
            type_parameters,
            heritage_clauses,
            mut members,
            members_loc,
            members_range,
        ) = {
            let source = self.store_for(node);
            let modifiers = source
                .source_modifiers(node)
                .map(ast::SourceModifierListInput::from_source);
            let name = source.name(node);
            let type_parameters = source
                .source_type_parameters(node)
                .map(ast::SourceNodeListInput::from_source);
            let heritage_clauses = source
                .source_heritage_clauses(node)
                .map(ast::SourceNodeListInput::from_source);
            let members = source
                .source_members(node)
                .expect("class expression should have members");
            let members_loc = members.loc();
            let members_range = members.range();
            let members = members.iter().collect::<Vec<_>>();
            (
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
                members_loc,
                members_range,
            )
        };

        let modifiers = modifiers.map(|modifiers| {
            self.import_state.preserve_source_modifier_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &modifiers,
            )
        });
        let name = name.map(|name| self.preserve_node(name));
        let type_parameters = type_parameters.map(|type_parameters| {
            self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &type_parameters,
            )
        });
        let heritage_clauses = heritage_clauses.map(|heritage_clauses| {
            self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &heritage_clauses,
            )
        });

        for member in &mut members {
            *member = self.preserve_node(*member);
        }
        let insertion_index = members
            .iter()
            .position(|member| self.is_class_this_assignment_block(*member))
            .map(|index| index + 1)
            .unwrap_or(0);
        members.insert(
            insertion_index,
            self.create_class_named_evaluation_helper_block(assigned_name),
        );
        let members = self
            .factory_mut()
            .new_node_list(members_loc, members_range, members);

        let updated = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_class_expression(
                node,
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_class_expression_from_store(
                source,
                node,
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
            )
        };
        self.emit_context
            .set_assigned_name(&updated, &assigned_name);
        if let Some(class_this) = self.emit_context.class_this(&node) {
            self.emit_context.set_class_this(&updated, &class_this);
        }
        updated
    }

    fn restore_outer_expressions(
        &mut self,
        outer_expression: ast::Node,
        inner: ast::Node,
    ) -> ast::Node {
        let source = self.store_for(outer_expression);
        if !ast::is_outer_expression(source, outer_expression, ast::OuterExpressionKinds::ALL) {
            return inner;
        }
        let Some(expression) = source.expression(outer_expression) else {
            return inner;
        };
        let restored = self.restore_outer_expressions(expression, inner);
        let outer_expression = self.preserve_node(outer_expression);
        let source = self.store_for(outer_expression);
        match source.kind(outer_expression) {
            ast::Kind::ParenthesizedExpression => self
                .factory_mut()
                .update_parenthesized_expression(outer_expression, restored),
            ast::Kind::TypeAssertionExpression => {
                let type_node = source
                    .r#type(outer_expression)
                    .expect("type assertion should have type");
                self.factory_mut()
                    .update_type_assertion(outer_expression, type_node, restored)
            }
            ast::Kind::AsExpression => {
                let type_node = source
                    .r#type(outer_expression)
                    .expect("as expression should have type");
                self.factory_mut()
                    .update_as_expression(outer_expression, restored, type_node)
            }
            ast::Kind::SatisfiesExpression => {
                let type_node = source
                    .r#type(outer_expression)
                    .expect("satisfies expression should have type");
                self.factory_mut().update_satisfies_expression(
                    outer_expression,
                    restored,
                    type_node,
                )
            }
            ast::Kind::NonNullExpression => {
                let flags = source.flags(outer_expression);
                self.factory_mut()
                    .update_non_null_expression(outer_expression, restored, flags)
            }
            ast::Kind::ExpressionWithTypeArguments => {
                let type_arguments = source
                    .source_type_arguments(outer_expression)
                    .map(|nodes| (nodes.loc(), nodes.range(), nodes.iter().collect::<Vec<_>>()));
                let type_arguments = type_arguments
                    .map(|(loc, range, nodes)| self.factory_mut().new_node_list(loc, range, nodes));
                self.factory_mut().update_expression_with_type_arguments(
                    outer_expression,
                    restored,
                    type_arguments,
                )
            }
            ast::Kind::PartiallyEmittedExpression => self
                .factory_mut()
                .update_partially_emitted_expression(outer_expression, restored),
            _ => restored,
        }
    }

    fn finish_transform_named_evaluation(
        &mut self,
        expression: ast::Node,
        assigned_name: ast::Node,
    ) -> ast::Node {
        let source = self.store_for(expression);
        let inner = ast::skip_outer_expressions(source, expression, ast::OuterExpressionKinds::ALL);
        let inner_source = self.store_for(inner);
        let updated = if ast::is_class_expression(inner_source, inner) {
            self.inject_class_named_evaluation_helper_block_if_missing(inner, assigned_name)
        } else {
            let inner = self.preserve_node(inner);
            self.emit_context
                .factory
                .new_set_function_name_helper(inner, assigned_name, "")
        };
        self.restore_outer_expressions(expression, updated)
    }

    fn get_assigned_name_of_identifier(
        &mut self,
        name: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        let expression_source = self.store_for(expression);
        let inner = ast::skip_outer_expressions(
            expression_source,
            expression,
            ast::OuterExpressionKinds::ALL,
        );
        let original = self.emit_context.most_original(&inner);
        let original_source = self.store_for(original);
        if (ast::is_class_declaration(original_source, original)
            || ast::is_function_declaration(original_source, original))
            && original_source.name(original).is_none()
            && ast::has_syntactic_modifier(original_source, original, ast::ModifierFlags::DEFAULT)
        {
            return self
                .factory_mut()
                .new_string_literal("default", ast::TokenFlags::NONE);
        }
        self.new_string_literal_from_node(name)
    }

    fn new_string_literal_from_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            return self
                .emit_context
                .factory
                .new_string_literal_from_node(source, &node);
        }
        let source = self.store_for(node);
        let text = source.text(node).to_owned();
        self.factory_mut()
            .new_string_literal(text, ast::TokenFlags::NONE)
    }

    fn transform_named_evaluation_of_variable_declaration(&mut self, node: ast::Node) -> ast::Node {
        // 14.3.1.2 RS: Evaluation
        //   LexicalBinding : BindingIdentifier Initializer
        //     ...
        //     3. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //        a. Let _value_ be ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...
        //
        // 14.3.2.1 RS: Evaluation
        //   VariableDeclaration : BindingIdentifier Initializer
        //     ...
        //     3. If IsAnonymousFunctionDefinition(|Initializer|) is *true*, then
        //        a. Let _value_ be ? NamedEvaluation of |Initializer| with argument _bindingId_.
        //     ...
        let source = self.store_for(node);
        let name = source
            .name(node)
            .expect("NamedEvaluation variable declaration should have a name");
        let initializer = source
            .initializer(node)
            .expect("NamedEvaluation variable declaration should have initializer");
        let assigned_name = self.get_assigned_name_of_identifier(name, initializer);
        let name = self.preserve_node(name);
        let initializer = self.finish_transform_named_evaluation(initializer, assigned_name);
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_variable_declaration(node, name, None, None, initializer)
        } else {
            let source = self.source;
            self.factory_mut().update_variable_declaration_from_store(
                source,
                node,
                name,
                None,
                None,
                initializer,
            )
        }
    }

    fn transform_named_evaluation_of_assignment_expression(
        &mut self,
        node: ast::Node,
    ) -> ast::Node {
        // 13.15.2 RS: Evaluation
        //   AssignmentExpression : LeftHandSideExpression `=` AssignmentExpression
        //     1. If |LeftHandSideExpression| is neither an |ObjectLiteral| nor an |ArrayLiteral|, then
        //        a. Let _lref_ be ? Evaluation of |LeftHandSideExpression|.
        //        b. If IsAnonymousFunctionDefinition(|AssignmentExpression|) and IsIdentifierRef of |LeftHandSideExpression| are both *true*, then
        //           i. Let _rval_ be ? NamedEvaluation of |AssignmentExpression| with argument _lref_.[[ReferencedName]].
        //     ...
        //
        //   AssignmentExpression : LeftHandSideExpression `&&=` AssignmentExpression
        //     ...
        //     5. If IsAnonymousFunctionDefinition(|AssignmentExpression|) is *true* and IsIdentifierRef of |LeftHandSideExpression| is *true*, then
        //        a. Let _rval_ be ? NamedEvaluation of |AssignmentExpression| with argument _lref_.[[ReferencedName]].
        //     ...
        //
        //   AssignmentExpression : LeftHandSideExpression `||=` AssignmentExpression
        //     ...
        //     5. If IsAnonymousFunctionDefinition(|AssignmentExpression|) is *true* and IsIdentifierRef of |LeftHandSideExpression| is *true*, then
        //        a. Let _rval_ be ? NamedEvaluation of |AssignmentExpression| with argument _lref_.[[ReferencedName]].
        //     ...
        //
        //   AssignmentExpression : LeftHandSideExpression `??=` AssignmentExpression
        //     ...
        //     4. If IsAnonymousFunctionDefinition(|AssignmentExpression|) is *true* and IsIdentifierRef of |LeftHandSideExpression| is *true*, then
        //        a. Let _rval_ be ? NamedEvaluation of |AssignmentExpression| with argument _lref_.[[ReferencedName]].
        //     ...
        let source = self.store_for(node);
        let left = source
            .left(node)
            .expect("NamedEvaluation assignment should have left side");
        let right = source
            .right(node)
            .expect("NamedEvaluation assignment should have right side");
        let operator_token = source
            .operator_token(node)
            .map(|operator| self.preserve_node(operator));
        let assigned_name = self.get_assigned_name_of_identifier(left, right);
        let left = self.preserve_node(left);
        let right = self.finish_transform_named_evaluation(right, assigned_name);
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_binary_expression(
                node,
                None::<ast::ModifierList>,
                Some(left),
                None::<ast::Node>,
                operator_token,
                Some(right),
            )
        } else {
            let source = self.source;
            self.factory_mut().update_binary_expression_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                Some(left),
                None::<ast::Node>,
                operator_token,
                Some(right),
            )
        }
    }

    fn transform_named_evaluation(
        &mut self,
        node: ast::Node,
        assigned_name: Option<&str>,
    ) -> ast::Node {
        match self.store_for(node).kind(node) {
            ast::Kind::VariableDeclaration => {
                self.transform_named_evaluation_of_variable_declaration(node)
            }
            ast::Kind::BinaryExpression => {
                if let Some(assigned_name) = assigned_name {
                    let assigned_name = self
                        .factory_mut()
                        .new_string_literal(assigned_name, ast::TokenFlags::NONE);
                    let right = self
                        .store_for(node)
                        .right(node)
                        .expect("NamedEvaluation assignment should have right side");
                    let right = self.finish_transform_named_evaluation(right, assigned_name);
                    let (left, operator_token) = {
                        let source = self.store_for(node);
                        (source.left(node), source.operator_token(node))
                    };
                    let left = left.map(|left| self.preserve_node(left));
                    let operator_token =
                        operator_token.map(|operator| self.preserve_node(operator));
                    if node.store_id() == self.factory().store().store_id() {
                        self.factory_mut().update_binary_expression(
                            node,
                            None::<ast::ModifierList>,
                            left,
                            None::<ast::Node>,
                            operator_token,
                            Some(right),
                        )
                    } else {
                        let source = self.source;
                        self.factory_mut().update_binary_expression_from_store(
                            source,
                            node,
                            None::<ast::ModifierList>,
                            left,
                            None::<ast::Node>,
                            operator_token,
                            Some(right),
                        )
                    }
                } else {
                    self.transform_named_evaluation_of_assignment_expression(node)
                }
            }
            _ => node,
        }
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        if !source
            .subtree_facts(*node)
            .contains(ast::SubtreeFacts::CONTAINS_USING)
        {
            return Some(*node);
        }

        match source.kind(*node) {
            ast::Kind::SourceFile => Some(self.visit_source_file(*node)),
            ast::Kind::Block => Some(self.visit_block(*node)),
            ast::Kind::ForStatement => Some(self.visit_for_statement(*node)),
            ast::Kind::ForOfStatement => Some(self.visit_for_of_statement(*node)),
            _ => Some(self.generated_visit_each_child(node)),
        }
    }

    fn visit_source_file(&mut self, node: ast::Node) -> ast::Node {
        let Some((
            original_statements,
            statements_loc,
            statements_range,
            using_kind,
            prologue_count,
            end_of_file_token,
        )) = ({
            let source = self.store_for(node);
            if source.as_source_file(node).is_declaration_file() {
                return node;
            }
            source.source_statements(node).map(|source_statements| {
                let original_statements = source_statements.iter().collect::<Vec<_>>();
                let using_kind = get_using_kind_of_statements(source, &original_statements);
                let prologue_count = original_statements
                    .iter()
                    .position(|statement| !ast::is_prologue_directive(source, *statement))
                    .unwrap_or(original_statements.len());
                let end_of_file_token = source.as_source_file(node).end_of_file_token();
                (
                    original_statements,
                    source_statements.loc(),
                    source_statements.range(),
                    using_kind,
                    prologue_count,
                    end_of_file_token,
                )
            })
        })
        else {
            return self.generated_visit_each_child(&node);
        };

        if using_kind == UsingKind::None {
            return self.generated_visit_each_child(&node);
        }

        // Imports and exports must stay at the top level. This means we must hoist all imports, exports, and
        // top-level function declarations and bindings out of the `try` statements we generate. For example:
        //
        // given:
        //
        //  import { w } from "mod";
        //  const x = expr1;
        //  using y = expr2;
        //  const z = expr3;
        //  export function f() {
        //    console.log(z);
        //  }
        //
        // produces:
        //
        //  import { x } from "mod";        // <-- preserved
        //  const x = expr1;                // <-- preserved
        //  var y, z;                       // <-- hoisted
        //  export function f() {           // <-- hoisted
        //    console.log(z);
        //  }
        //  const env_1 = { stack: [], error: void 0, hasError: false };
        //  try {
        //    y = __addDisposableResource(env_1, expr2, false);
        //    z = expr3;
        //  }
        //  catch (e_1) {
        //    env_1.error = e_1;
        //    env_1.hasError = true;
        //  }
        //  finally {
        //    __disposeResource(env_1);
        //  }
        //
        // In this transformation, we hoist `y`, `z`, and `f` to a new outer statement list while moving all other
        // statements in the source file into the `try` block, which is the same approach we use for System module
        // emit. Unlike System module emit, we attempt to preserve all statements prior to the first top-level
        // `using` to isolate the complexity of the transformed output to only where it is necessary.
        self.emit_context.start_variable_environment();

        self.export_bindings.clear();
        self.export_vars.clear();

        let (prologue, rest) = original_statements.split_at(prologue_count);
        let mut top_level_statements = self.visit_slice(prologue);

        // Collect and transform any leading statements up to the first `using` or `await using`. This preserves
        // the original statement order much as is possible.
        let mut pos = 0;
        while pos < rest.len() {
            let statement = rest[pos];
            if get_using_kind(self.store_for(statement), statement) != UsingKind::None {
                if pos > 0 {
                    top_level_statements.extend(self.visit_slice(&rest[..pos]));
                }
                break;
            }
            pos += 1;
        }

        if pos >= rest.len() {
            panic!("Should have encountered at least one 'using' statement.");
        }

        let env_binding = self.create_env_binding();
        let body_statements = self.transform_using_declarations(
            &rest[pos..],
            env_binding,
            Some(&mut top_level_statements),
        );

        if !self.export_bindings.is_empty() {
            let specifiers = self.export_bindings.values().copied().collect::<Vec<_>>();
            let specifier_list = self.emit_context.factory.new_node_list(specifiers);
            let named_exports = self.factory_mut().new_named_exports(specifier_list);
            top_level_statements.push(self.factory_mut().new_export_declaration(
                None::<ast::ModifierList>,
                false,
                Some(named_exports),
                None::<ast::Node>,
                None::<ast::Node>,
            ));
        }

        top_level_statements.extend(self.emit_context.end_variable_environment());
        if !self.export_vars.is_empty() {
            let export_modifier = self.factory_mut().new_modifier(ast::Kind::ExportKeyword);
            let modifier_list = self
                .emit_context
                .factory
                .new_modifier_list(vec![export_modifier]);
            let declarations = self
                .emit_context
                .factory
                .new_node_list(self.export_vars.clone());
            let list = self
                .factory_mut()
                .new_variable_declaration_list(declarations, ast::NodeFlags::LET);
            top_level_statements.push(
                self.factory_mut()
                    .new_variable_statement(Some(modifier_list), list),
            );
        }
        top_level_statements.extend(self.create_downlevel_using_statements(
            body_statements,
            env_binding,
            using_kind == UsingKind::Async,
        ));

        if let Some(export_equals_binding) = self.export_equals_binding {
            top_level_statements.push(self.factory_mut().new_export_assignment(
                None::<ast::ModifierList>,
                true,
                None::<ast::Node>,
                Some(export_equals_binding),
            ));
        }

        let statement_list = self.factory_mut().new_node_list(
            statements_loc,
            statements_range,
            top_level_statements,
        );
        let end_of_file_token =
            end_of_file_token.map(|end_of_file_token| self.preserve_node(end_of_file_token));
        let visited = self.update_source_file_from_visited(
            node,
            Some(statement_list),
            end_of_file_token,
            false,
        );
        self.emit_context.add_requested_emit_helpers(&visited);
        self.export_vars.clear();
        self.export_bindings.clear();
        self.default_export_binding = None;
        self.export_equals_binding = None;
        visited
    }

    fn visit_block(&mut self, node: ast::Node) -> ast::Node {
        let source_is_output = node.store_id() == self.factory().store().store_id();
        let Some((original_statements, loc, range, multi_line, using_kind, prologue_count)) = ({
            let source = self.store_for(node);
            source.source_statements(node).map(|source_statements| {
                let original_statements = source_statements.iter().collect::<Vec<_>>();
                let using_kind = get_using_kind_of_statements(source, &original_statements);
                let prologue_count = original_statements
                    .iter()
                    .position(|statement| !ast::is_prologue_directive(source, *statement))
                    .unwrap_or(original_statements.len());
                (
                    original_statements,
                    source_statements.loc(),
                    source_statements.range(),
                    source.multi_line(node).unwrap_or(true),
                    using_kind,
                    prologue_count,
                )
            })
        }) else {
            return self.generated_visit_each_child(&node);
        };
        if using_kind == UsingKind::None {
            return self.generated_visit_each_child(&node);
        }

        let (prologue, rest) = original_statements.split_at(prologue_count);
        let env_binding = self.create_env_binding();
        let mut statements = self.visit_slice(prologue);
        let body = self.transform_using_declarations(rest, env_binding, None);
        statements.extend(self.create_downlevel_using_statements(
            body,
            env_binding,
            using_kind == UsingKind::Async,
        ));
        let statement_list = self.factory_mut().new_node_list(loc, range, statements);
        if source_is_output {
            self.factory_mut()
                .update_block(node, statement_list, multi_line)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_block_from_store(source, node, statement_list, multi_line)
        }
    }

    fn visit_for_statement(&mut self, node: ast::Node) -> ast::Node {
        let source_is_output = node.store_id() == self.factory().store().store_id();
        let Some((initializer, condition, incrementor, statement)) = ({
            let source = self.store_for(node);
            source.initializer(node).map(|initializer| {
                (
                    initializer,
                    source.condition(node),
                    source.incrementor(node),
                    source.statement(node),
                )
            })
        }) else {
            return self.generated_visit_each_child(&node);
        };
        if !is_using_variable_declaration_list(self.store_for(initializer), initializer) {
            return self.generated_visit_each_child(&node);
        }

        // given:
        //
        //  for (using x = expr; cond; incr) { ... }
        //
        // produces a shallow transformation to:
        //
        //  {
        //    using x = expr;
        //    for (; cond; incr) { ... }
        //  }
        //
        // before handing the shallow transformation back to the visitor for an in-depth transformation.
        let condition = condition.map(|node| self.preserve_node(node));
        let incrementor = incrementor.map(|node| self.preserve_node(node));
        let statement = statement.map(|node| self.preserve_node(node));
        let for_statement = if source_is_output {
            self.factory_mut().update_for_statement(
                node,
                None::<ast::Node>,
                condition,
                incrementor,
                statement,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_for_statement_from_store(
                source,
                node,
                None::<ast::Node>,
                condition,
                incrementor,
                statement,
            )
        };
        let initializer = self.preserve_node(initializer);
        let variable_statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, initializer);
        let block_statements = self
            .emit_context
            .factory
            .new_node_list(vec![variable_statement, for_statement]);
        let block = self.factory_mut().new_block(block_statements, false);
        self.visit_node(Some(block)).unwrap_or(block)
    }

    fn visit_for_of_statement(&mut self, node: ast::Node) -> ast::Node {
        let node_source = self.store_for(node);
        let Some(initializer) = node_source.initializer(node) else {
            return if node.store_id() == self.factory().store().store_id() {
                node
            } else {
                self.generated_visit_each_child(&node)
            };
        };
        if !is_using_variable_declaration_list(self.store_for(initializer), initializer) {
            return if node.store_id() == self.factory().store().store_id() {
                self.visit_for_of_statement_children(node)
            } else {
                self.generated_visit_each_child(&node)
            };
        }

        // given:
        //
        //  for (using x of y) { ... }
        //
        // produces a shallow transformation to:
        //
        //  for (const x_1 of y) {
        //    using x = x;
        //    ...
        //  }
        //
        // before handing the shallow transformation back to the visitor for an in-depth transformation.
        let initializer_source = self.store_for(initializer);
        let is_await_using =
            get_using_kind_of_variable_declaration_list(initializer_source, initializer)
                == UsingKind::Async;
        let declarations = initializer_source
            .declarations(initializer)
            .map(|declarations| declarations.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let for_decl = if let Some(for_decl) = declarations.first().copied() {
            for_decl
        } else {
            let temp = self.emit_context.factory.new_temp_variable();
            self.factory_mut().new_variable_declaration(
                temp,
                None::<ast::Node>,
                None::<ast::Node>,
                None::<ast::Node>,
            )
        };
        let original_name = self.store_for(for_decl).name(for_decl);
        let name = original_name.map(|name| self.preserve_node(name));
        let temp = if let Some(original_name) = original_name {
            self.emit_context.new_generated_name_for_node(original_name)
        } else {
            self.emit_context.factory.new_temp_variable()
        };
        let using_var = if for_decl.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_variable_declaration(
                for_decl,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                Some(temp),
            )
        } else {
            let source = self.source;
            self.factory_mut().update_variable_declaration_from_store(
                source,
                for_decl,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                Some(temp),
            )
        };
        let using_var_declarations = self.emit_context.factory.new_node_list(vec![using_var]);
        let using_var_list = self.factory_mut().new_variable_declaration_list(
            using_var_declarations,
            if is_await_using {
                ast::NodeFlags::AWAIT_USING
            } else {
                ast::NodeFlags::USING
            },
        );
        let using_var_statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, using_var_list);

        let original_statement = self
            .store_for(node)
            .statement(node)
            .expect("for-of statement should have body");
        let original_statement_source = self.store_for(original_statement);
        let statement = if ast::is_block(original_statement_source, original_statement) {
            let source_statements = self
                .store_for(original_statement)
                .source_statements(original_statement)
                .expect("block should have statements");
            let loc = source_statements.loc();
            let range = source_statements.range();
            let multi_line = self
                .store_for(original_statement)
                .multi_line(original_statement)
                .unwrap_or(true);
            let mut statements = vec![using_var_statement];
            let original_nodes = source_statements.iter().collect::<Vec<_>>();
            statements.extend(
                original_nodes
                    .into_iter()
                    .map(|node| self.preserve_node(node)),
            );
            let list = self.factory_mut().new_node_list(loc, range, statements);
            if original_statement.store_id() == self.factory().store().store_id() {
                self.factory_mut()
                    .update_block(original_statement, list, multi_line)
            } else {
                let source = self.source;
                self.factory_mut().update_block_from_store(
                    source,
                    original_statement,
                    list,
                    multi_line,
                )
            }
        } else {
            let original_statement = self.preserve_node(original_statement);
            let statements = self
                .emit_context
                .factory
                .new_node_list(vec![using_var_statement, original_statement]);
            self.factory_mut().new_block(statements, true)
        };

        let for_decl = self.factory_mut().new_variable_declaration(
            temp,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let for_declarations = self.emit_context.factory.new_node_list(vec![for_decl]);
        let for_initializer = self
            .factory_mut()
            .new_variable_declaration_list(for_declarations, ast::NodeFlags::CONST);
        let await_modifier = self
            .store_for(node)
            .await_modifier(node)
            .map(|node| self.preserve_node(node));
        let expression = self
            .store_for(node)
            .expression(node)
            .map(|node| self.preserve_node(node));
        let for_of = if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_for_in_or_of_statement(
                node,
                await_modifier,
                Some(for_initializer),
                expression,
                Some(statement),
            )
        } else {
            let source = self.source;
            self.factory_mut().update_for_in_or_of_statement_from_store(
                source,
                node,
                await_modifier,
                Some(for_initializer),
                expression,
                Some(statement),
            )
        };
        self.visit_node(Some(for_of)).unwrap_or(for_of)
    }

    fn visit_for_of_statement_children(&mut self, node: ast::Node) -> ast::Node {
        let (await_modifier, initializer, expression, statement) = {
            let source = self.store_for(node);
            (
                source.await_modifier(node),
                source.initializer(node),
                source.expression(node),
                source.statement(node),
            )
        };
        let await_modifier = await_modifier.map(|node| self.preserve_node(node));
        let initializer = initializer.map(|node| self.visit_node(Some(node)).unwrap_or(node));
        let expression = expression.map(|node| self.visit_node(Some(node)).unwrap_or(node));
        let statement = statement.map(|node| self.visit_node(Some(node)).unwrap_or(node));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_for_in_or_of_statement(
                node,
                await_modifier,
                initializer,
                expression,
                statement,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_for_in_or_of_statement_from_store(
                source,
                node,
                await_modifier,
                initializer,
                expression,
                statement,
            )
        }
    }

    fn transform_using_declarations(
        &mut self,
        statements_in: &[ast::Node],
        env_binding: ast::Node,
        mut top_level_statements: Option<&mut Vec<ast::Node>>,
    ) -> Vec<ast::Node> {
        let mut statements = Vec::new();
        for statement in statements_in {
            let using_kind = get_using_kind(self.store_for(*statement), *statement);
            if using_kind != UsingKind::None {
                let var_statement = *statement;
                let declaration_list = self
                    .store_for(var_statement)
                    .declaration_list(var_statement)
                    .expect("using statement should have declaration list");
                let declaration_source = self.store_for(declaration_list);
                let source_declarations = declaration_source
                    .declarations(declaration_list)
                    .map(|declarations| declarations.iter().collect::<Vec<_>>())
                    .unwrap_or_default();
                let mut declarations = Vec::new();
                let mut valid = true;
                for declaration in source_declarations {
                    let declaration_source = self.store_for(declaration);
                    let Some(name) = declaration_source.name(declaration) else {
                        valid = false;
                        break;
                    };
                    if !ast::is_identifier(self.store_for(name), name) {
                        // Since binding patterns are a grammar error, we reset `declarations` so we don't process this as a `using`.
                        valid = false;
                        break;
                    }
                    let declaration = if self.is_named_evaluation(declaration) {
                        self.transform_named_evaluation_of_variable_declaration(declaration)
                    } else {
                        declaration
                    };
                    let declaration_source = self.store_for(declaration);
                    let initializer =
                        if let Some(initializer) = declaration_source.initializer(declaration) {
                            self.visit_node(Some(initializer)).unwrap_or(initializer)
                        } else {
                            self.emit_context.factory.new_void_zero_expression()
                        };
                    let name = self.preserve_node(name);
                    let initializer = self
                        .emit_context
                        .factory
                        .new_add_disposable_resource_helper(
                            env_binding,
                            initializer,
                            using_kind == UsingKind::Async,
                        );
                    let declaration = if declaration.store_id() == self.factory().store().store_id()
                    {
                        self.factory_mut().update_variable_declaration(
                            declaration,
                            Some(name),
                            None::<ast::Node>,
                            None::<ast::Node>,
                            Some(initializer),
                        )
                    } else {
                        let source = self.source;
                        self.factory_mut().update_variable_declaration_from_store(
                            source,
                            declaration,
                            Some(name),
                            None::<ast::Node>,
                            None::<ast::Node>,
                            Some(initializer),
                        )
                    };
                    declarations.push(declaration);
                }

                if valid && !declarations.is_empty() {
                    let declaration_nodes = self.emit_context.factory.new_node_list(declarations);
                    let var_list = self
                        .factory_mut()
                        .new_variable_declaration_list(declaration_nodes, ast::NodeFlags::CONST);
                    self.emit_context.set_original(&var_list, &declaration_list);
                    let updated = if var_statement.store_id() == self.factory().store().store_id() {
                        self.factory_mut().update_variable_statement(
                            var_statement,
                            None::<ast::ModifierList>,
                            var_list,
                        )
                    } else {
                        let source = self.source;
                        self.factory_mut().update_variable_statement_from_store(
                            source,
                            var_statement,
                            None::<ast::ModifierList>,
                            var_list,
                        )
                    };
                    self.hoist_or_append_node(
                        updated,
                        top_level_statements.as_deref_mut(),
                        &mut statements,
                    );
                    continue;
                }
            }

            if let Some(result) = self.visit_node(Some(*statement)) {
                let result_source = self.store_for(result);
                if result_source.kind(result) == ast::Kind::SyntaxList {
                    let children = result_source
                        .syntax_list_children(result)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    for child in children {
                        self.hoist_or_append_node(
                            child,
                            top_level_statements.as_deref_mut(),
                            &mut statements,
                        );
                    }
                } else {
                    self.hoist_or_append_node(
                        result,
                        top_level_statements.as_deref_mut(),
                        &mut statements,
                    );
                }
            }
        }
        statements
    }

    fn hoist_or_append_node(
        &mut self,
        node: ast::Node,
        top_level_statements: Option<&mut Vec<ast::Node>>,
        statements: &mut Vec<ast::Node>,
    ) {
        if let Some(top_level_statements) = top_level_statements {
            if let Some(node) = self.hoist(node, top_level_statements) {
                let node = self.preserve_node(node);
                statements.push(node);
            }
        } else {
            let node = self.preserve_node(node);
            statements.push(node);
        }
    }

    fn hoist(
        &mut self,
        node: ast::Node,
        top_level_statements: &mut Vec<ast::Node>,
    ) -> Option<ast::Node> {
        match self.store_for(node).kind(node) {
            ast::Kind::ImportDeclaration
            | ast::Kind::ImportEqualsDeclaration
            | ast::Kind::ExportDeclaration
            | ast::Kind::FunctionDeclaration => {
                self.hoist_import_or_export_or_hoisted_declaration(node, top_level_statements);
                None
            }
            ast::Kind::ExportAssignment => Some(self.hoist_export_assignment(node)),
            ast::Kind::ClassDeclaration => Some(self.hoist_class_declaration(node)),
            ast::Kind::VariableStatement => self.hoist_variable_statement(node),
            _ => Some(node),
        }
    }

    fn hoist_import_or_export_or_hoisted_declaration(
        &mut self,
        node: ast::Node,
        top_level_statements: &mut Vec<ast::Node>,
    ) {
        // NOTE: `node` has already been visited
        let node = self.preserve_node(node);
        top_level_statements.push(node);
    }

    fn hoist_export_assignment(&mut self, node: ast::Node) -> ast::Node {
        if self.store_for(node).is_export_equals(node).unwrap_or(false) {
            self.hoist_export_equals(node)
        } else {
            self.hoist_export_default(node)
        }
    }

    fn hoist_export_default(&mut self, node: ast::Node) -> ast::Node {
        // NOTE: `node` has already been visited
        if self.default_export_binding.is_some() {
            // invalid case of multiple `export default` declarations. Don't assert here, just pass it through
            return node;
        }

        // given:
        //
        //   export default expr;
        //
        // produces:
        //
        //   // top level
        //   var default_1;
        //   export { default_1 as default };
        //
        //   // body
        //   default_1 = expr;
        let default_export_binding = self.default_export_name();
        self.default_export_binding = Some(default_export_binding);
        let default_name = self.factory_mut().new_identifier("default");
        self.hoist_binding_identifier(default_export_binding, true, Some(default_name), Some(node));

        // give a class or function expression an assigned name, if needed.
        let expression = self
            .store_for(node)
            .expression(node)
            .map(|expression| self.preserve_node(expression))
            .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
        let expression = {
            let source = self.store_for(expression);
            let inner_expression =
                ast::skip_outer_expressions(source, expression, ast::OuterExpressionKinds::ALL);
            if self.is_named_evaluation(inner_expression) {
                let inner_expression =
                    self.transform_named_evaluation(inner_expression, Some("default"));
                self.restore_outer_expressions(expression, inner_expression)
            } else {
                expression
            }
        };
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(default_export_binding, expression);
        self.factory_mut().new_expression_statement(assignment)
    }

    fn hoist_export_equals(&mut self, node: ast::Node) -> ast::Node {
        // NOTE: `node` has already been visited
        if self.export_equals_binding.is_some() {
            // invalid case of multiple `export default` declarations. Don't assert here, just pass it through
            return node;
        }

        // given:
        //
        //   export = expr;
        //
        // produces:
        //
        //   // top level
        //   var default_1;
        //
        //   try {
        //       // body
        //       default_1 = expr;
        //   } ...
        //
        //   // top level suffix
        //   export = default_1;
        let export_equals_binding = self.default_export_name();
        self.export_equals_binding = Some(export_equals_binding);
        self.emit_context
            .add_variable_declaration(export_equals_binding);
        let expression = self
            .store_for(node)
            .expression(node)
            .map(|expression| self.preserve_node(expression))
            .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(export_equals_binding, expression);
        self.factory_mut().new_expression_statement(assignment)
    }

    fn hoist_class_declaration(&mut self, node: ast::Node) -> ast::Node {
        // NOTE: `node` has already been visited
        if self.store_for(node).name(node).is_none() && self.default_export_binding.is_some() {
            // invalid case of multiple `export default` declarations. Don't assert here, just pass it through
            return node;
        }

        let is_exported =
            ast::has_syntactic_modifier(self.store_for(node), node, ast::ModifierFlags::EXPORT);
        let is_default =
            ast::has_syntactic_modifier(self.store_for(node), node, ast::ModifierFlags::DEFAULT);
        let mut expression = self.convert_class_declaration_to_class_expression(node);
        if self.store_for(node).name(node).is_some() {
            // given:
            //
            //  using x = expr;
            //  class C {}
            //
            // produces:
            //
            //  var x, C;
            //  const env_1 = { ... };
            //  try {
            //    x = __addDisposableResource(env_1, expr, false);
            //    C = class {};
            //  }
            //  catch (e_1) {
            //    env_1.error = e_1;
            //    env_1.hasError = true;
            //  }
            //  finally {
            //    __disposeResources(env_1);
            //  }
            //
            // If the class is exported, we also produce an `export { C };`
            let local_name = if node.store_id() == self.factory().store().store_id() {
                self.emit_context
                    .factory
                    .get_local_name_of_factory_node(&node)
            } else {
                let source = self.source;
                self.emit_context.factory.get_local_name(source, &node)
            };
            self.hoist_binding_identifier(local_name, is_exported && !is_default, None, Some(node));
            let declaration_name = if is_default {
                if node.store_id() == self.factory().store().store_id() {
                    self.emit_context
                        .factory
                        .get_local_name_of_factory_node(&node)
                } else {
                    let source = self.source;
                    self.emit_context.factory.get_local_name(source, &node)
                }
            } else if node.store_id() == self.factory().store().store_id() {
                let name = self
                    .factory()
                    .store()
                    .name(node)
                    .expect("class declaration should have name");
                let name = self
                    .factory_mut()
                    .deep_clone_node_in_current_store_preserve_location(name);
                self.emit_context
                    .mark_emit_node(&name, printer::EF_NO_COMMENTS | printer::EF_NO_SOURCE_MAP);
                name
            } else {
                let source = self.source;
                self.emit_context
                    .factory
                    .get_declaration_name(source, &node)
            };
            expression = self
                .emit_context
                .factory
                .new_assignment_expression(declaration_name, expression);
            self.emit_context.set_original(&expression, &node);
            let loc = self.store_for(node).loc(node);
            self.emit_context.set_source_map_range(&expression, loc);
            self.emit_context.set_comment_range(&expression, loc);
            if self.is_named_evaluation(expression) {
                expression = self.transform_named_evaluation(expression, None);
            }
        }

        if is_default && self.default_export_binding.is_none() {
            // In the case of a default export, we create a temporary variable that we export as the default and then
            // assign to that variable.
            //
            // given:
            //
            //  using x = expr;
            //  export default class C {}
            //
            // produces:
            //
            //  export { default_1 as default };
            //  var x, C, default_1;
            //  const env_1 = { ... };
            //  try {
            //    x = __addDisposableResource(env_1, expr, false);
            //    default_1 = C = class {};
            //  }
            //  catch (e_1) {
            //    env_1.error = e_1;
            //    env_1.hasError = true;
            //  }
            //  finally {
            //    __disposeResources(env_1);
            //  }
            //
            // Though we will never reassign `default_1`, this most closely matches the specified runtime semantics.
            let default_export_binding = self.default_export_name();
            self.default_export_binding = Some(default_export_binding);
            let default_name = self.factory_mut().new_identifier("default");
            self.hoist_binding_identifier(
                default_export_binding,
                true,
                Some(default_name),
                Some(node),
            );
            expression = self
                .emit_context
                .factory
                .new_assignment_expression(default_export_binding, expression);
            self.emit_context.set_original(&expression, &node);
            if self.is_named_evaluation(expression) {
                expression = self.transform_named_evaluation(expression, Some("default"));
            }
        }

        self.factory_mut().new_expression_statement(expression)
    }

    fn hoist_variable_statement(&mut self, node: ast::Node) -> Option<ast::Node> {
        // NOTE: `node` has already been visited
        let mut expressions = Vec::new();
        let is_exported =
            ast::has_syntactic_modifier(self.store_for(node), node, ast::ModifierFlags::EXPORT);
        let declaration_list = self.store_for(node).declaration_list(node)?;
        let variables = self
            .store_for(declaration_list)
            .declarations(declaration_list)
            .map(|declarations| declarations.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for variable in variables {
            self.hoist_binding_element(variable, is_exported, Some(variable));
            if self.store_for(variable).initializer(variable).is_some() {
                expressions.push(self.hoist_initialized_variable(variable));
            }
        }
        if !expressions.is_empty() {
            let inline = self
                .emit_context
                .factory
                .inline_expressions(&expressions)
                .expect("expressions should not be empty");
            let statement = self.factory_mut().new_expression_statement(inline);
            self.emit_context.set_original(&statement, &node);
            let loc = self.store_for(node).loc(node);
            self.emit_context.set_comment_range(&statement, loc);
            self.emit_context.set_source_map_range(&statement, loc);
            Some(statement)
        } else {
            None
        }
    }

    fn hoist_initialized_variable(&mut self, node: ast::Node) -> ast::Node {
        // NOTE: `node` has already been visited
        let initializer = self
            .store_for(node)
            .initializer(node)
            .expect("Expected initializer");
        let name = self
            .store_for(node)
            .name(node)
            .expect("variable declaration should have name");
        let target = if ast::is_identifier(self.store_for(name), name) {
            let target = self.preserve_node(name);
            let flags = self.emit_context.emit_flags(&target)
                & !(printer::EF_LOCAL_NAME | printer::EF_EXPORT_NAME);
            self.emit_context.set_emit_flags(&target, flags);
            target
        } else {
            self.convert_binding_pattern_to_assignment_pattern(name)
        };
        let initializer = self.preserve_node(initializer);
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(target, initializer);
        self.emit_context.set_original(&assignment, &node);
        let loc = self.store_for(node).loc(node);
        self.emit_context.set_comment_range(&assignment, loc);
        self.emit_context.set_source_map_range(&assignment, loc);
        assignment
    }

    fn hoist_binding_element(
        &mut self,
        node: ast::Node,
        is_exported_declaration: bool,
        original: Option<ast::Node>,
    ) {
        // NOTE: `node` has already been visited
        let Some(name) = self.store_for(node).name(node) else {
            return;
        };
        if ast::is_binding_pattern(self.store_for(name), name) {
            let elements = self
                .store_for(name)
                .elements(name)
                .map(|elements| elements.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            for element in elements {
                if self.store_for(element).name(element).is_some() {
                    self.hoist_binding_element(element, is_exported_declaration, original);
                }
            }
        } else {
            self.hoist_binding_identifier(name, is_exported_declaration, None, original);
        }
    }

    fn hoist_binding_identifier(
        &mut self,
        node: ast::Node,
        is_export: bool,
        export_alias: Option<ast::Node>,
        original: Option<ast::Node>,
    ) {
        // NOTE: `node` has already been visited
        let name = if !utilities::is_generated_identifier(self.emit_context, &node) {
            self.preserve_node(node)
        } else {
            node
        };
        if is_export {
            if export_alias.is_none() && !utilities::is_local_name(self.emit_context, &name) {
                let var_decl = self.factory_mut().new_variable_declaration(
                    name,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    None::<ast::Node>,
                );
                if let Some(original) = original {
                    self.emit_context.set_original(&var_decl, &original);
                }
                self.export_vars.push(var_decl);
                return;
            }

            let (local_name, export_name) = if let Some(export_alias) = export_alias {
                (Some(name), export_alias)
            } else {
                (None, name)
            };
            let specifier =
                self.factory_mut()
                    .new_export_specifier(false, local_name, Some(export_name));
            if let Some(original) = original {
                self.emit_context.set_original(&specifier, &original);
            }
            let key = self.store_for(name).text(name).to_owned();
            self.export_bindings.insert(key, specifier);
        }
        self.emit_context.add_variable_declaration(name);
    }

    fn create_env_binding(&mut self) -> ast::Node {
        self.emit_context.factory.new_unique_name("env")
    }

    fn create_downlevel_using_statements(
        &mut self,
        body_statements: Vec<ast::Node>,
        env_binding: ast::Node,
        r#async: bool,
    ) -> Vec<ast::Node> {
        let mut statements = Vec::with_capacity(2);

        // produces:
        //
        //  const env_1 = { stack: [], error: void 0, hasError: false };
        //
        let empty_array_elements = self.emit_context.factory.new_node_list(Vec::new());
        let empty_array = self
            .factory_mut()
            .new_array_literal_expression(empty_array_elements, false);
        let stack_name = self.factory_mut().new_identifier("stack");
        let stack = self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            Some(stack_name),
            None::<ast::Node>,
            None::<ast::Node>,
            Some(empty_array),
        );
        let error_name = self.factory_mut().new_identifier("error");
        let error_initializer = self.emit_context.factory.new_void_zero_expression();
        let error = self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            Some(error_name),
            None::<ast::Node>,
            None::<ast::Node>,
            Some(error_initializer),
        );
        let has_error_name = self.factory_mut().new_identifier("hasError");
        let false_expr = self.emit_context.factory.new_false_expression();
        let has_error = self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            Some(has_error_name),
            None::<ast::Node>,
            None::<ast::Node>,
            Some(false_expr),
        );
        let env_properties = self
            .emit_context
            .factory
            .new_node_list(vec![stack, error, has_error]);
        let env_object = self
            .factory_mut()
            .new_object_literal_expression(env_properties, false);
        let env_var = self.factory_mut().new_variable_declaration(
            env_binding,
            None::<ast::Node>,
            None::<ast::Node>,
            Some(env_object),
        );
        let env_declarations = self.emit_context.factory.new_node_list(vec![env_var]);
        let env_var_list = self
            .factory_mut()
            .new_variable_declaration_list(env_declarations, ast::NodeFlags::CONST);
        statements.push(
            self.factory_mut()
                .new_variable_statement(None::<ast::ModifierList>, env_var_list),
        );

        // when `async` is `false`, produces:
        //
        //  try {
        //    <bodyStatements>
        //  }
        //  catch (e_1) {
        //      env_1.error = e_1;
        //      env_1.hasError = true;
        //  }
        //  finally {
        //    __disposeResources(env_1);
        //  }
        //
        // when `async` is `true`, produces:
        //
        //  try {
        //    <bodyStatements>
        //  }
        //  catch (e_1) {
        //      env_1.error = e_1;
        //      env_1.hasError = true;
        //  }
        //  finally {
        //    const result_1 = __disposeResources(env_1);
        //    if (result_1) {
        //      await result_1;
        //    }
        //  }
        //
        // Unfortunately, it is necessary to use two properties to indicate an error because `throw undefined` is legal
        // JavaScript.
        let try_statements = self.emit_context.factory.new_node_list(body_statements);
        let try_block = self.factory_mut().new_block(try_statements, true);
        let body_catch_binding = self.emit_context.factory.new_unique_name("e");
        let catch_declaration = self.factory_mut().new_variable_declaration(
            body_catch_binding,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let error_name = self.factory_mut().new_identifier("error");
        let has_error_name = self.factory_mut().new_identifier("hasError");
        let error_access = self.factory_mut().new_property_access_expression(
            env_binding,
            None::<ast::Node>,
            error_name,
            ast::NodeFlags::NONE,
        );
        let error_assignment = self
            .emit_context
            .factory
            .new_assignment_expression(error_access, body_catch_binding);
        let error_statement = self
            .factory_mut()
            .new_expression_statement(error_assignment);
        let has_error_access = self.factory_mut().new_property_access_expression(
            env_binding,
            None::<ast::Node>,
            has_error_name,
            ast::NodeFlags::NONE,
        );
        let true_expr = self.emit_context.factory.new_true_expression();
        let has_error_assignment = self
            .emit_context
            .factory
            .new_assignment_expression(has_error_access, true_expr);
        let has_error_statement = self
            .factory_mut()
            .new_expression_statement(has_error_assignment);
        let catch_statements = self
            .emit_context
            .factory
            .new_node_list(vec![error_statement, has_error_statement]);
        let catch_block = self.factory_mut().new_block(catch_statements, true);
        let catch_clause = self
            .factory_mut()
            .new_catch_clause(Some(catch_declaration), Some(catch_block));

        let finally_block = if r#async {
            let result = self.emit_context.factory.new_unique_name("result");
            let dispose = self
                .emit_context
                .factory
                .new_dispose_resources_helper(env_binding);
            let result_decl = self.factory_mut().new_variable_declaration(
                result,
                None::<ast::Node>,
                None::<ast::Node>,
                Some(dispose),
            );
            let result_declarations = self.emit_context.factory.new_node_list(vec![result_decl]);
            let result_list = self
                .factory_mut()
                .new_variable_declaration_list(result_declarations, ast::NodeFlags::CONST);
            let result_statement = self
                .factory_mut()
                .new_variable_statement(None::<ast::ModifierList>, result_list);
            let await_result = self.factory_mut().new_await_expression(result);
            let await_statement = self.factory_mut().new_expression_statement(await_result);
            let if_statement = self.factory_mut().new_if_statement(
                Some(result),
                Some(await_statement),
                None::<ast::Node>,
            );
            let finally_statements = self
                .emit_context
                .factory
                .new_node_list(vec![result_statement, if_statement]);
            self.factory_mut().new_block(finally_statements, true)
        } else {
            let dispose = self
                .emit_context
                .factory
                .new_dispose_resources_helper(env_binding);
            let dispose_statement = self.factory_mut().new_expression_statement(dispose);
            let finally_statements = self
                .emit_context
                .factory
                .new_node_list(vec![dispose_statement]);
            self.factory_mut().new_block(finally_statements, true)
        };

        let try_statement = self.factory_mut().new_try_statement(
            Some(try_block),
            Some(catch_clause),
            Some(finally_block),
        );
        statements.push(try_statement);
        statements
    }

    fn convert_class_declaration_to_class_expression(&mut self, node: ast::Node) -> ast::Node {
        let (modifiers, name, type_parameters, heritage_clauses, members) =
            if node.store_id() == self.factory().store().store_id() {
                let (modifiers, name, type_parameters, heritage_clauses, members) = {
                    let source = self.factory().store();
                    let modifiers = source
                        .source_modifiers(node)
                        .map(|modifiers| modifiers.iter().collect::<Vec<_>>());
                    let name = source.name(node);
                    let type_parameters = source.source_type_parameters(node).map(|nodes| {
                        (nodes.loc(), nodes.range(), nodes.iter().collect::<Vec<_>>())
                    });
                    let heritage_clauses = source.source_heritage_clauses(node).map(|nodes| {
                        (nodes.loc(), nodes.range(), nodes.iter().collect::<Vec<_>>())
                    });
                    let members = source.source_members(node).map(|nodes| {
                        (nodes.loc(), nodes.range(), nodes.iter().collect::<Vec<_>>())
                    });
                    (modifiers, name, type_parameters, heritage_clauses, members)
                };
                let modifiers = modifiers
                    .map(|modifiers| self.emit_context.factory.new_modifier_list(modifiers));
                let type_parameters = type_parameters
                    .map(|(loc, range, nodes)| self.factory_mut().new_node_list(loc, range, nodes));
                let heritage_clauses = heritage_clauses
                    .map(|(loc, range, nodes)| self.factory_mut().new_node_list(loc, range, nodes));
                let members = members
                    .map(|(loc, range, nodes)| self.factory_mut().new_node_list(loc, range, nodes));
                (modifiers, name, type_parameters, heritage_clauses, members)
            } else {
                let source = self.source;
                let source_modifiers = source.source_modifiers(node);
                let source_name = source.name(node);
                let source_type_parameters = source.source_type_parameters(node);
                let source_heritage_clauses = source.source_heritage_clauses(node);
                let source_members = source.source_members(node);
                let modifiers = self.visit_modifiers_input(
                    (source_modifiers).map(ast::SourceModifierListInput::from_source),
                );
                let name = source_name.map(|name| self.preserve_node(name));
                let type_parameters = self.visit_nodes_input(
                    (source_type_parameters).map(ast::SourceNodeListInput::from_source),
                );
                let heritage_clauses = self.visit_nodes_input(
                    (source_heritage_clauses).map(ast::SourceNodeListInput::from_source),
                );
                let members = self
                    .visit_nodes_input((source_members).map(ast::SourceNodeListInput::from_source));
                (modifiers, name, type_parameters, heritage_clauses, members)
            };
        let expression = self.factory_mut().new_class_expression(
            modifiers,
            name,
            type_parameters,
            heritage_clauses,
            members.expect("class declaration members must exist"),
        );
        self.emit_context.set_original(&expression, &node);
        expression
    }

    fn convert_binding_pattern_to_assignment_pattern(&mut self, pattern: ast::Node) -> ast::Node {
        match self.source.kind(pattern) {
            ast::Kind::ArrayBindingPattern => {
                let mut elements = Vec::new();
                if let Some(source_elements) = self.source.source_elements(pattern) {
                    let loc = source_elements.loc();
                    let range = source_elements.range();
                    for element in source_elements.iter().collect::<Vec<_>>() {
                        elements.push(
                            self.convert_binding_element_to_array_assignment_element(element),
                        );
                    }
                    let list = self.factory_mut().new_node_list(loc, range, elements);
                    let array = self.factory_mut().new_array_literal_expression(list, false);
                    self.emit_context.set_original(&array, &pattern);
                    self.emit_context
                        .set_source_map_range(&array, self.source.loc(pattern));
                    array
                } else {
                    let list = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        elements,
                    );
                    self.factory_mut().new_array_literal_expression(list, false)
                }
            }
            ast::Kind::ObjectBindingPattern => {
                let mut properties = Vec::new();
                if let Some(source_elements) = self.source.source_elements(pattern) {
                    let loc = source_elements.loc();
                    let range = source_elements.range();
                    for element in source_elements.iter().collect::<Vec<_>>() {
                        properties.push(
                            self.convert_binding_element_to_object_assignment_element(element),
                        );
                    }
                    let list = self.factory_mut().new_node_list(loc, range, properties);
                    let object = self
                        .factory_mut()
                        .new_object_literal_expression(list, false);
                    self.emit_context.set_original(&object, &pattern);
                    self.emit_context
                        .set_source_map_range(&object, self.source.loc(pattern));
                    object
                } else {
                    let list = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        properties,
                    );
                    self.factory_mut()
                        .new_object_literal_expression(list, false)
                }
            }
            _ => panic!("unknown binding pattern"),
        }
    }

    fn convert_binding_element_to_array_assignment_element(
        &mut self,
        element: ast::Node,
    ) -> ast::Node {
        let Some(name) = self.source.name(element) else {
            let omitted = self.factory_mut().new_omitted_expression();
            self.emit_context.set_original(&omitted, &element);
            self.emit_context
                .set_source_map_range(&omitted, self.source.loc(element));
            return omitted;
        };
        if self.source.dot_dot_dot_token(element).is_some() {
            let name = self.preserve_node(name);
            let spread = self.factory_mut().new_spread_element(name);
            self.emit_context.set_original(&spread, &element);
            self.emit_context
                .set_source_map_range(&spread, self.source.loc(element));
            return spread;
        }
        let mut expression = self.convert_binding_name_to_assignment_element_target(name);
        if let Some(initializer) = self.source.initializer(element) {
            let initializer = self.preserve_node(initializer);
            expression = self
                .emit_context
                .factory
                .new_assignment_expression(expression, initializer);
            self.emit_context.set_original(&expression, &element);
            self.emit_context
                .set_source_map_range(&expression, self.source.loc(element));
        }
        expression
    }

    fn convert_binding_element_to_object_assignment_element(
        &mut self,
        element: ast::Node,
    ) -> ast::Node {
        let name = self.source.name(element);
        if self.source.dot_dot_dot_token(element).is_some() {
            let name = name.map(|name| self.preserve_node(name));
            let spread = self.factory_mut().new_spread_assignment(name);
            self.emit_context.set_original(&spread, &element);
            self.emit_context
                .set_source_map_range(&spread, self.source.loc(element));
            return spread;
        }
        if let Some(property_name) = self.source.property_name(element) {
            let original_name = self
                .source
                .name(element)
                .expect("binding element should have name");
            let mut expression =
                self.convert_binding_name_to_assignment_element_target(original_name);
            if let Some(initializer) = self.source.initializer(element) {
                let initializer = self.preserve_node(initializer);
                expression = self
                    .emit_context
                    .factory
                    .new_assignment_expression(expression, initializer);
            }
            let property_name = self.preserve_node(property_name);
            let assignment = self.factory_mut().new_property_assignment(
                None::<ast::ModifierList>,
                Some(property_name),
                None::<ast::Node>,
                None::<ast::Node>,
                Some(expression),
            );
            self.emit_context.set_original(&assignment, &element);
            self.emit_context
                .set_source_map_range(&assignment, self.source.loc(element));
            return assignment;
        }
        let equals_token = if self.source.initializer(element).is_some() {
            Some(self.factory_mut().new_token(ast::Kind::EqualsToken))
        } else {
            None
        };
        let name = name.map(|name| self.preserve_node(name));
        let initializer = self
            .source
            .initializer(element)
            .map(|node| self.preserve_node(node));
        let assignment = self.factory_mut().new_shorthand_property_assignment(
            None::<ast::ModifierList>,
            name,
            None::<ast::Node>,
            None::<ast::Node>,
            equals_token,
            initializer,
        );
        self.emit_context.set_original(&assignment, &element);
        self.emit_context
            .set_source_map_range(&assignment, self.source.loc(element));
        assignment
    }

    fn convert_binding_name_to_assignment_element_target(&mut self, name: ast::Node) -> ast::Node {
        if ast::is_binding_pattern(self.source, name) {
            self.convert_binding_pattern_to_assignment_pattern(name)
        } else {
            self.preserve_node(name)
        }
    }

    fn default_export_name(&mut self) -> ast::Node {
        self.emit_context.factory.new_unique_name_ex(
            "_default",
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES
                    | GeneratedIdentifierFlags::FILE_LEVEL
                    | GeneratedIdentifierFlags::OPTIMISTIC,
                ..Default::default()
            },
        )
    }

    fn visit_slice(&mut self, nodes: &[ast::Node]) -> Vec<ast::Node> {
        nodes
            .iter()
            .filter_map(|node| self.visit_node(Some(*node)))
            .collect()
    }

    fn append_visited_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
        changed: &mut bool,
    ) {
        match visited {
            Some(visited) if self.preserved_source_node_matches(Some(original), Some(visited)) => {
                out.push(self.preserve_node(original));
            }
            Some(visited) => {
                *changed = true;
                let store = self.store_for(visited);
                if store.kind(visited) == ast::Kind::SyntaxList {
                    let nodes = store
                        .syntax_list_children(visited)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    for node in nodes {
                        out.push(self.preserve_node(node));
                    }
                } else {
                    out.push(self.preserve_node(visited));
                }
            }
            None => *changed = true,
        }
    }
}

fn is_using_variable_declaration_list(source: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_variable_declaration_list(source, node)
        && get_using_kind_of_variable_declaration_list(source, node) != UsingKind::None
}

fn get_using_kind_of_variable_declaration_list(
    source: &ast::AstStore,
    node: ast::Node,
) -> UsingKind {
    match source.flags(node) & ast::NodeFlags::BLOCK_SCOPED {
        flags if flags == ast::NodeFlags::AWAIT_USING => UsingKind::Async,
        flags if flags == ast::NodeFlags::USING => UsingKind::Sync,
        _ => UsingKind::None,
    }
}

fn get_using_kind_of_variable_statement(source: &ast::AstStore, node: ast::Node) -> UsingKind {
    source
        .declaration_list(node)
        .map(|declaration_list| {
            get_using_kind_of_variable_declaration_list(source, declaration_list)
        })
        .unwrap_or(UsingKind::None)
}

fn get_using_kind(source: &ast::AstStore, statement: ast::Node) -> UsingKind {
    if ast::is_variable_statement(source, statement) {
        get_using_kind_of_variable_statement(source, statement)
    } else {
        UsingKind::None
    }
}

fn get_using_kind_of_statements(source: &ast::AstStore, statements: &[ast::Node]) -> UsingKind {
    let mut result = UsingKind::None;
    for statement in statements {
        let using_kind = get_using_kind(source, *statement);
        if using_kind == UsingKind::Async {
            return UsingKind::Async;
        }
        if using_kind > result {
            result = using_kind;
        }
    }
    result
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for UsingDeclarationTransformer<'_, 'source> {
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        self.import_state.preserved_node(self.factory(), source)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return node;
        }
        let source = self.source;
        self.import_state
            .preserve_node(source, &mut self.emit_context.factory.node_factory, node)
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        let imported = self.preserve_node(imported);
        self.import_state.record_preserved_node(
            source.store_id(),
            &mut self.emit_context.factory.node_factory,
            source,
            imported,
        )
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        self.import_state
            .preserved_source_node_matches(self.factory(), source, output)
    }

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: Option<ast::NodeList>,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            if source_unchanged {
                return node;
            }
            return self.factory_mut().update_source_file_in_current_store(
                node,
                statements.expect("source file statements cannot be removed"),
                end_of_file_token,
            );
        }
        let source = self.source;
        if source_unchanged {
            let imported = self.preserve_node(node);
            return self.record_preserved_node(node, imported);
        }
        self.import_state.update_source_file_from_store(
            source,
            &mut self.emit_context.factory.node_factory,
            node,
            statements.expect("source file statements cannot be removed"),
            end_of_file_token,
        )
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let mut visited = self.visit(&node)?;
        let store = self.store_for(visited);
        if store.kind(visited) == ast::Kind::SyntaxList {
            let mut nodes = store
                .syntax_list_children(visited)
                .expect("SyntaxList should have children")
                .iter();
            let visited_slot = nodes
                .next()
                .expect("expected only a single node to be written to output");
            assert!(
                nodes.next().is_none(),
                "expected only a single node to be written to output"
            );
            visited = visited_slot?;
        }
        Some(self.preserve_node(visited))
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_nodes_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;
        for node in source_list.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        if changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                visited,
                source_list.has_trailing_comma(),
            ))
        } else {
            Some(self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            ))
        }
    }

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        Some(self.import_state.preserve_source_modifier_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &modifiers,
        ))
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        Some(self.import_state.preserve_source_raw_node_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &nodes,
        ))
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source>
    for UsingDeclarationTransformer<'_, 'source>
{
}
