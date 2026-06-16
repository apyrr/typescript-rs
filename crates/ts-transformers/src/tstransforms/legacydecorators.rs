use std::collections::HashMap;

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

use crate::utilities::is_generated_identifier;

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    facts: &LegacyDecoratorsResolverFacts,
) -> ast::Node {
    let mut runtime = LegacyDecoratorsRuntime {
        source: source_file.store(),
        emit_context,
        language_version: compiler_options.get_emit_script_target(),
        facts,
        import_state: ast::AstImportState::new(),
        class_aliases: HashMap::new(),
        enclosing_classes: Vec::new(),
    };
    let root = runtime.visit_node(Some(root)).unwrap_or(root);
    runtime.emit_context.add_requested_emit_helpers(&root);
    root
}

#[derive(Clone, Default)]
pub struct LegacyDecoratorsResolverFacts {
    referenced_value_declarations: HashMap<ast::Node, ast::Node>,
}

impl LegacyDecoratorsResolverFacts {
    fn referenced_value_declaration(&self, node: ast::Node) -> Option<ast::Node> {
        self.referenced_value_declarations.get(&node).copied()
    }
}

pub fn collect_legacy_decorators_resolver_facts(
    source_file: &ast::SourceFile,
    resolver: &mut dyn printer::EmitResolver,
) -> LegacyDecoratorsResolverFacts {
    let mut facts = LegacyDecoratorsResolverFacts::default();
    let store = source_file.store();
    let mut stack = vec![source_file.root()];
    while let Some(node) = stack.pop() {
        if ast::is_identifier(store, node)
            && let Some(declaration) = resolver.get_referenced_value_declaration(node)
        {
            facts
                .referenced_value_declarations
                .insert(node, declaration);
        }
        let _ = store.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }
    facts
}

struct LegacyDecoratorsRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    language_version: core::ScriptTarget,
    facts: &'source LegacyDecoratorsResolverFacts,
    import_state: ast::AstImportState,
    /**
     * A map that keeps track of aliases created for classes with decorators to avoid issues
     * with the double-binding behavior of classes.
     */
    class_aliases: HashMap<ast::Node, ast::Node>,
    enclosing_classes: Vec<ast::Node>,
}

impl LegacyDecoratorsRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn emit_factory_mut(&mut self) -> &mut printer::NodeFactory {
        &mut self.emit_context.factory
    }

    fn preserve_optional_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.preserve_node(node))
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn name_of_declaration(&self, node: ast::Node) -> Option<ast::Node> {
        let store = self.store_for(node);
        ast::get_name_of_declaration(store, Some(node))
    }

    fn local_name_for_declaration(&mut self, node: ast::Node) -> ast::Node {
        self.name_for_declaration_with_emit_flags(
            node,
            printer::EF_LOCAL_NAME | printer::EF_NO_COMMENTS,
        )
    }

    fn declaration_name(&mut self, node: ast::Node) -> ast::Node {
        self.name_for_declaration_with_emit_flags(
            node,
            printer::EF_NO_COMMENTS | printer::EF_NO_SOURCE_MAP,
        )
    }

    fn declaration_name_without_comments(&mut self, node: ast::Node) -> ast::Node {
        self.name_for_declaration_with_emit_flags(node, printer::EF_NO_COMMENTS)
    }

    fn name_for_declaration_with_emit_flags(
        &mut self,
        node: ast::Node,
        emit_flags: printer::EmitFlags,
    ) -> ast::Node {
        if let Some(name) = self.name_of_declaration(node) {
            let name = if name.store_id() == self.factory().store().store_id() {
                self.factory_mut()
                    .deep_clone_node_in_current_store_preserve_location(name)
            } else {
                let source = self.source;
                self.factory_mut()
                    .deep_clone_node_from_store_preserve_location(source, name)
            };
            self.emit_context.mark_emit_node(&name, emit_flags);
            return name;
        }

        let original = self.emit_context.most_original(&node);
        self.emit_context
            .factory
            .new_generated_name_for_node(self.source, &original)
    }

    fn modifiers_without_export_default_or_decorators(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::ModifierList> {
        let modifier_data = {
            let store = self.store_for(node);
            let modifiers = store.source_modifiers(node)?;
            if modifiers.is_empty() {
                return None;
            }
            let modifier_nodes = modifiers
                .nodes()
                .iter()
                .filter(|modifier| !is_export_or_default_or_decorator(store, *modifier))
                .collect::<Vec<_>>();
            let flags = if modifier_nodes.len() == modifiers.nodes().len() {
                modifiers.modifier_flags()
            } else {
                ast::ModifierFlags::NONE
            };
            (modifiers.loc(), modifiers.range(), modifier_nodes, flags)
        };
        Some(self.factory_mut().new_modifier_list(
            modifier_data.0,
            modifier_data.1,
            modifier_data.2,
            modifier_data.3,
        ))
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        // we have to visit all identifiers in classes, just in case they require substitution
        if !source
            .subtree_facts(*node)
            .contains(ast::SubtreeFacts::CONTAINS_DECORATORS)
            && self.enclosing_classes.is_empty()
        {
            return Some(*node);
        }
        match source.kind(*node) {
            ast::Kind::Identifier => Some(self.visit_identifier(*node)),
            ast::Kind::PropertyAccessExpression => {
                Some(self.visit_property_access_expression(*node))
            }
            ast::Kind::Decorator => {
                // Decorators are elided. They will be emitted as part of `visitClassDeclaration`.
                None
            }
            ast::Kind::ClassDeclaration => Some(self.visit_class_declaration(*node)),
            ast::Kind::ClassExpression => Some(self.visit_class_expression(*node)),
            ast::Kind::Constructor => Some(self.visit_constructor_declaration(*node)),
            ast::Kind::MethodDeclaration => Some(self.visit_method_declaration(*node)),
            ast::Kind::SetAccessor => Some(self.visit_set_accessor_declaration(*node)),
            ast::Kind::GetAccessor => Some(self.visit_get_accessor_declaration(*node)),
            ast::Kind::PropertyDeclaration => self.visit_property_declaration(*node),
            ast::Kind::Parameter => Some(self.visit_parameter_declaration(*node)),
            _ => Some(self.generated_visit_each_child(node)),
        }
    }

    fn visit_identifier(&mut self, node: ast::Node) -> ast::Node {
        // takes the place of `substituteIdentifier` in the strada transform
        for class in self.enclosing_classes.iter().copied() {
            if let Some(class_alias) = self.class_aliases.get(&class).copied()
                && self
                    .referenced_value_declaration(node)
                    .is_some_and(|declaration| {
                        self.emit_context.most_original(&declaration)
                            == self.emit_context.most_original(&class)
                    })
            {
                return class_alias;
            }
        }
        node
    }

    fn visit_property_access_expression(&mut self, node: ast::Node) -> ast::Node {
        // Visit the expression but not the name, since property access names should not be substituted.
        // Strada's onSubstituteNode only fires for EmitHint.Expression, which excludes the
        // .name of PropertyAccessExpression.
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (expression_input, question_dot_token_input, name_input, flags) = {
            let source = self.store_for(node);
            (
                source.expression(node),
                source.question_dot_token(node),
                source.name(node),
                source.flags(node),
            )
        };
        let expression = self.visit_node(expression_input);
        if !self.preserved_source_node_matches(expression_input, expression) {
            let question_dot_token = self.preserve_optional_node(question_dot_token_input);
            let name = self.preserve_optional_node(name_input);
            return if is_factory_node {
                self.factory_mut().update_property_access_expression(
                    node,
                    expression,
                    question_dot_token,
                    name,
                    flags,
                )
            } else {
                let source = self.source;
                self.factory_mut()
                    .update_property_access_expression_from_store(
                        source,
                        node,
                        expression,
                        question_dot_token,
                        name,
                        flags,
                    )
            };
        }
        node
    }

    fn empty_modifiers(&mut self, node: ast::Node) -> Option<ast::ModifierList> {
        let (loc, range) = {
            let source = self.store_for(node);
            let modifiers = source.source_modifiers(node)?;
            (modifiers.loc(), modifiers.range())
        };
        Some(self.factory_mut().new_modifier_list(
            loc,
            range,
            Vec::<ast::Node>::new(),
            ast::ModifierFlags::NONE,
        ))
    }

    fn visit_parameter_declaration(&mut self, node: ast::Node) -> ast::Node {
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (name_input, initializer_input, dot_dot_dot_token_input, loc) = {
            let source = self.store_for(node);
            (
                source.name(node),
                source.initializer(node),
                source.dot_dot_dot_token(node),
                source.loc(node),
            )
        };
        let name = self.visit_node(name_input);
        let initializer = self.visit_node(initializer_input);
        let empty_modifiers = self.empty_modifiers(node);
        let dot_dot_dot_token = self.preserve_optional_node(dot_dot_dot_token_input);
        let updated = if is_factory_node {
            self.factory_mut().update_parameter_declaration(
                node,
                empty_modifiers,
                dot_dot_dot_token,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_parameter_declaration_from_store(
                source,
                node,
                empty_modifiers,
                dot_dot_dot_token,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        };
        if updated != node {
            // While we emit the source map for the node after skipping decorators and modifiers,
            // we need to emit the comments for the original range.
            self.emit_context.set_comment_range(&updated, loc);
            let original = self.emit_context.most_original(&node);
            let source = self.store_for(original);
            let new_loc = move_range_past_modifiers(source, original);
            self.factory_mut().place_transformed_node(updated, new_loc);
            self.emit_context.set_source_map_range(&updated, new_loc);
            if let Some(name) = self.factory().store().name(updated) {
                self.emit_context
                    .set_emit_flags(&name, printer::EF_NO_TRAILING_SOURCE_MAP);
            }
        }
        updated
    }

    fn visit_constructor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (parameters_input, body_input, modifiers_input) = if is_factory_node {
            let source = self.factory().store();
            (
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        } else {
            let source = self.source;
            (
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        };
        let parameters = self
            .visit_parameters_input(parameters_input)
            .expect("constructor parameters must exist");
        let body = self.visit_function_body(body_input);
        let modifiers = self.visit_modifiers_input(modifiers_input);
        if is_factory_node {
            self.factory_mut().update_constructor_declaration(
                node,
                modifiers,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_constructor_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
        }
    }

    fn visit_method_declaration(&mut self, node: ast::Node) -> ast::Node {
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (parameters_input, body_input, modifiers_input, asterisk_token_input) =
            if is_factory_node {
                let source = self.factory().store();
                (
                    source
                        .source_parameters(node)
                        .map(ast::SourceNodeListInput::from_source),
                    source.body(node),
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.asterisk_token(node),
                )
            } else {
                let source = self.source;
                (
                    source
                        .source_parameters(node)
                        .map(ast::SourceNodeListInput::from_source),
                    source.body(node),
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.asterisk_token(node),
                )
            };
        let parameters = self
            .visit_parameters_input(parameters_input)
            .expect("method parameters must exist");
        let body = self.visit_function_body(body_input);
        let name = self.visit_property_name_of_class_element(node);
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let asterisk_token = self.preserve_optional_node(asterisk_token_input);
        let updated = if is_factory_node {
            self.factory_mut().update_method_declaration(
                node,
                modifiers,
                asterisk_token,
                name,
                None::<ast::Node>,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_method_declaration_from_store(
                source,
                node,
                modifiers,
                asterisk_token,
                name,
                None::<ast::Node>,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        };
        self.finish_class_element(updated, node)
    }

    fn visit_get_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (parameters_input, body_input, modifiers_input) = if is_factory_node {
            let source = self.factory().store();
            (
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        } else {
            let source = self.source;
            (
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        };
        let parameters = self
            .visit_parameters_input(parameters_input)
            .expect("accessor parameters must exist");
        let body = self.visit_function_body(body_input);
        let name = self.visit_property_name_of_class_element(node);
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let updated = if is_factory_node {
            self.factory_mut().update_get_accessor_declaration(
                node,
                modifiers,
                name,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_get_accessor_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
        };
        self.finish_class_element(updated, node)
    }

    fn visit_set_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (parameters_input, body_input, modifiers_input) = if is_factory_node {
            let source = self.factory().store();
            (
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        } else {
            let source = self.source;
            (
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        };
        let parameters = self
            .visit_parameters_input(parameters_input)
            .expect("accessor parameters must exist");
        let body = self.visit_function_body(body_input);
        let name = self.visit_property_name_of_class_element(node);
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let updated = if is_factory_node {
            self.factory_mut().update_set_accessor_declaration(
                node,
                modifiers,
                name,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_set_accessor_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
        };
        self.finish_class_element(updated, node)
    }

    fn visit_property_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (is_ambient, has_ambient_or_abstract_modifier, modifiers_input, initializer) =
            if is_factory_node {
                let source = self.factory().store();
                (
                    source.flags(node).contains(ast::NodeFlags::AMBIENT),
                    ast::has_syntactic_modifier(
                        source,
                        node,
                        ast::ModifierFlags::AMBIENT | ast::ModifierFlags::ABSTRACT,
                    ),
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.initializer(node),
                )
            } else {
                let source = self.source;
                (
                    source.flags(node).contains(ast::NodeFlags::AMBIENT),
                    ast::has_syntactic_modifier(
                        source,
                        node,
                        ast::ModifierFlags::AMBIENT | ast::ModifierFlags::ABSTRACT,
                    ),
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.initializer(node),
                )
            };
        if is_ambient {
            return None;
        }
        if has_ambient_or_abstract_modifier {
            return None;
        }

        let modifiers = self.visit_modifiers_input(modifiers_input);
        let name = self.visit_property_name_of_class_element(node);
        let initializer = self.visit_node(initializer);
        let updated = if is_factory_node {
            self.factory_mut().update_property_declaration(
                node,
                modifiers,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_property_declaration_from_store(
                source,
                node,
                modifiers,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        };
        Some(self.finish_class_element(updated, node))
    }

    fn visit_class_expression(&mut self, node: ast::Node) -> ast::Node {
        // Legacy decorators were not supported on class expressions
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let heritage_clauses_input = if is_factory_node {
            self.factory()
                .store()
                .source_heritage_clauses(node)
                .map(ast::SourceNodeListInput::from_source)
        } else {
            self.source
                .source_heritage_clauses(node)
                .map(ast::SourceNodeListInput::from_source)
        };
        let members_input = if is_factory_node {
            self.factory()
                .store()
                .source_members(node)
                .map(ast::SourceNodeListInput::from_source)
        } else {
            self.source
                .source_members(node)
                .map(ast::SourceNodeListInput::from_source)
        };
        let modifiers_input = if is_factory_node {
            self.factory()
                .store()
                .source_modifiers(node)
                .map(ast::SourceModifierListInput::from_source)
        } else {
            self.source
                .source_modifiers(node)
                .map(ast::SourceModifierListInput::from_source)
        };
        let name = if is_factory_node {
            self.factory().store().name(node)
        } else {
            self.source.name(node)
        };

        let heritage_clauses = self.visit_nodes_input(heritage_clauses_input);
        let members = self
            .visit_nodes_input(members_input)
            .expect("class expression members must exist");
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let name = self.preserve_optional_node(name);
        if is_factory_node {
            self.factory_mut().update_class_expression(
                node,
                modifiers,
                name,
                None,
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
                None,
                heritage_clauses,
                members,
            )
        }
    }

    fn visit_class_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let decorated = ast::class_or_constructor_parameter_is_decorated(source, true, node);
        if !(decorated
            || !get_decorated_class_elements(source, node, false).is_empty()
            || !get_decorated_class_elements(source, node, true).is_empty())
        {
            return self.generated_visit_each_child(&node);
        }

        if decorated {
            return self.transform_class_declaration_with_class_decorators(node);
        }
        self.transform_class_declaration_without_class_decorators(node)
    }

    /**
     * Transforms a non-decorated class declaration.
     *
     * @param node A ClassDeclaration node.
     * @param name The name of the class.
     */
    fn transform_class_declaration_without_class_decorators(
        &mut self,
        node: ast::Node,
    ) -> ast::Node {
        let is_factory_node = node.store_id() == self.factory().store().store_id();
        let (modifiers_input, heritage_clauses_input, members_input, existing_name) =
            if is_factory_node {
                let source = self.factory().store();
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source
                        .source_heritage_clauses(node)
                        .map(ast::SourceNodeListInput::from_source),
                    source
                        .source_members(node)
                        .map(ast::SourceNodeListInput::from_source),
                    source.name(node),
                )
            } else {
                let source = self.source;
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source
                        .source_heritage_clauses(node)
                        .map(ast::SourceNodeListInput::from_source),
                    source
                        .source_members(node)
                        .map(ast::SourceNodeListInput::from_source),
                    source.name(node),
                )
            };
        //  ${modifiers} class ${name} ${heritageClauses} {
        //      ${members}
        //  }
        let modifiers = self.visit_modifiers_input(modifiers_input);
        let heritage_clauses = self.visit_nodes_input(heritage_clauses_input);
        let initial_members = self
            .visit_nodes_input(members_input)
            .expect("class declaration members must exist");
        let (members, decoration_statements) =
            self.transform_decorators_of_class_elements(node, initial_members);

        let name = if existing_name.is_none() && !decoration_statements.is_empty() {
            Some(
                self.emit_context
                    .factory
                    .new_generated_name_for_node(self.source, &node),
            )
        } else {
            self.preserve_optional_node(existing_name)
        };
        let updated = if is_factory_node {
            self.factory_mut().update_class_declaration(
                node,
                modifiers,
                name,
                None::<ast::NodeList>,
                heritage_clauses,
                members,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_class_declaration_from_store(
                source,
                node,
                modifiers,
                name,
                None,
                heritage_clauses,
                members,
            )
        };

        if decoration_statements.is_empty() {
            return updated;
        }
        let mut statements = vec![updated];
        statements.extend(decoration_statements);
        self.factory_mut().new_syntax_list(statements)
    }

    /**
     * Transforms a decorated class declaration and appends the resulting statements. If
     * the class requires an alias to avoid issues with double-binding, the alias is returned.
     */
    fn transform_class_declaration_with_class_decorators(&mut self, node: ast::Node) -> ast::Node {
        // When we emit an ES6 class that has a class decorator, we must tailor the
        // emit to certain specific cases.
        //
        // In the simplest case, we emit the class declaration as a let declaration, and
        // evaluate decorators after the close of the class body:
        //
        //  [Example 1]
        //  ---------------------------------------------------------------------
        //  TypeScript                      | Javascript
        //  ---------------------------------------------------------------------
        //  @dec                            | let C = class C {
        //  class C {                       | }
        //  }                               | C = __decorate([dec], C);
        //  ---------------------------------------------------------------------
        //  @dec                            | let C = class C {
        //  export class C {                | }
        //  }                               | C = __decorate([dec], C);
        //                                  | export { C };
        //  ---------------------------------------------------------------------
        //
        // If a class declaration contains a reference to itself *inside* of the class body,
        // this introduces two bindings to the class: One outside of the class body, and one
        // inside of the class body. If we apply decorators as in [Example 1] above, there
        // is the possibility that the decorator `dec` will return a new value for the
        // constructor, which would result in the binding inside of the class no longer
        // pointing to the same reference as the binding outside of the class.
        //
        // As a result, we must instead rewrite all references to the class *inside* of the
        // class body to instead point to a local temporary alias for the class:
        //
        //  [Example 2]
        //  ---------------------------------------------------------------------
        //  TypeScript                      | Javascript
        //  ---------------------------------------------------------------------
        //  @dec                            | let C = C_1 = class C {
        //  class C {                       |   static x() { return C_1.y; }
        //    static x() { return C.y; }    | }
        //    static y = 1;                 | C.y = 1;
        //  }                               | C = C_1 = __decorate([dec], C);
        //                                  | var C_1;
        //  ---------------------------------------------------------------------
        //  @dec                            | let C = class C {
        //  export class C {                |   static x() { return C_1.y; }
        //    static x() { return C.y; }    | }
        //    static y = 1;                 | C.y = 1;
        //  }                               | C = C_1 = __decorate([dec], C);
        //                                  | export { C };
        //                                  | var C_1;
        //  ---------------------------------------------------------------------
        //
        // If a class declaration is the default export of a module, we instead emit
        // the export after the decorated declaration:
        //
        //  [Example 3]
        //  ---------------------------------------------------------------------
        //  TypeScript                      | Javascript
        //  ---------------------------------------------------------------------
        //  @dec                            | let default_1 = class {
        //  export default class {          | }
        //  }                               | default_1 = __decorate([dec], default_1);
        //                                  | export default default_1;
        //  ---------------------------------------------------------------------
        //  @dec                            | let C = class C {
        //  export default class C {        | }
        //  }                               | C = __decorate([dec], C);
        //                                  | export default C;
        //  ---------------------------------------------------------------------
        //
        // If the class declaration is the default export and a reference to itself
        // inside of the class body, we must emit both an alias for the class *and*
        // move the export after the declaration:
        //
        //  [Example 4]
        //  ---------------------------------------------------------------------
        //  TypeScript                      | Javascript
        //  ---------------------------------------------------------------------
        //  @dec                            | let C = class C {
        //  export default class C {        |   static x() { return C_1.y; }
        //    static x() { return C.y; }    | }
        //    static y = 1;                 | C.y = 1;
        //  }                               | C = C_1 = __decorate([dec], C);
        //                                  | export default C;
        //                                  | var C_1;
        //  ---------------------------------------------------------------------
        //

        let (is_export, is_default, location, original_loc, heritage_input, members_input) = {
            let source = self.store_for(node);
            let members = source
                .source_members(node)
                .expect("class declaration members must exist");
            (
                ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT),
                ast::has_syntactic_modifier(source, node, ast::ModifierFlags::DEFAULT),
                move_range_past_modifiers(source, node),
                source.loc(node),
                source
                    .source_heritage_clauses(node)
                    .map(ast::SourceNodeListInput::from_source),
                ast::SourceNodeListInput::from_source(members),
            )
        };
        let modifiers = self.modifiers_without_export_default_or_decorators(node);
        let class_alias = self.get_class_alias_if_needed(node);
        if class_alias.is_some() {
            self.push_enclosing_class(node);
        }

        // When we used to transform to ES5/3 this would be moved inside an IIFE and should reference the name
        // without any block-scoped variable collision handling - but we don't support that anymore, so we always
        // use the local name for the class
        let decl_name = self.local_name_for_declaration(node);

        //  ... = class ${name} ${heritageClauses} {
        //      ${members}
        //  }
        let heritage_clauses = self.visit_nodes_input(heritage_input);
        let members_loc = members_input.loc();
        let members_range = members_input.range();
        let members = self
            .visit_nodes_input(Some(members_input))
            .expect("class declaration members must exist");
        let (mut members, decoration_statements) =
            self.transform_decorators_of_class_elements(node, members);

        // If we're emitting to ES2022 or later then we need to reassign the class alias before
        // static initializers are evaluated.
        let assign_class_alias_in_static_block = class_alias.is_some()
            && self.language_version >= core::ScriptTarget::ES2022
            && members.iter(self.factory().store()).any(|member| {
                is_class_static_block_declaration_or_static_property(self.store_for(member), member)
            });
        if assign_class_alias_in_static_block {
            let class_alias = class_alias.expect("class alias should be defined");
            let this = self
                .factory_mut()
                .new_keyword_expression(ast::Kind::ThisKeyword);
            let assignment = self
                .emit_factory_mut()
                .new_assignment_expression(class_alias, this);
            let statement = self.factory_mut().new_expression_statement(assignment);
            self.emit_context
                .mark_emit_node(&assignment, printer::EF_NO_COMMENTS);
            self.emit_context
                .mark_emit_node(&statement, printer::EF_NO_COMMENTS);
            let synthesized = core::undefined_text_range();
            let statements =
                self.factory_mut()
                    .new_node_list(synthesized, synthesized, vec![statement]);
            let body = self.factory_mut().new_block(statements, false);
            let static_block = self
                .factory_mut()
                .new_class_static_block_declaration(None::<ast::ModifierList>, Some(body));
            self.emit_context
                .mark_emit_node(&static_block, printer::EF_NO_COMMENTS);
            let mut member_list = vec![static_block];
            member_list.extend(members.iter(self.factory().store()));
            members = self
                .factory_mut()
                .new_node_list(members_loc, members_range, member_list);
        }

        let class_name = {
            let source = self.store_for(node);
            source.name(node)
        };
        let class_name =
            class_name.filter(|name| !is_generated_identifier(self.emit_context, name));
        let class_name = self.preserve_optional_node(class_name);
        let class_expression = self.factory_mut().new_class_expression(
            modifiers,
            class_name,
            None::<ast::NodeList>,
            heritage_clauses,
            members,
        );

        self.factory_mut()
            .place_transformed_node(class_expression, location);
        self.emit_context.set_original(&class_expression, &node);

        //  let ${name} = ${classExpression} where name is either declaredName if the class doesn't contain self-reference
        //                                         or decoratedClassAlias if the class contain self-reference.
        let var_initializer = if let Some(class_alias) = class_alias
            && !assign_class_alias_in_static_block
        {
            self.emit_factory_mut()
                .new_assignment_expression(class_alias, class_expression)
        } else {
            class_expression
        };
        let var_decl =
            self.factory_mut()
                .new_variable_declaration(decl_name, None, None, var_initializer);
        self.emit_context.set_original(&var_decl, &node);

        let var_decl_list = self.emit_factory_mut().new_node_list(vec![var_decl]);
        let var_decl_list = self
            .factory_mut()
            .new_variable_declaration_list(var_decl_list, ast::NodeFlags::LET);
        let var_statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, var_decl_list);
        self.emit_context.set_original(&var_statement, &node);
        self.factory_mut()
            .place_transformed_node(var_statement, location);
        self.emit_context
            .set_comment_range(&var_statement, original_loc);

        let mut statements = vec![var_statement];
        statements.extend(decoration_statements);
        let has_constructor_decoration = self.get_constructor_decoration_statement(node);
        if let Some(constructor_decoration) = has_constructor_decoration {
            statements.push(constructor_decoration);
        }

        if is_export {
            let export_statement = if is_default {
                self.emit_factory_mut().new_export_default(decl_name)
            } else {
                let declaration_name = self.declaration_name(node);
                self.emit_factory_mut()
                    .new_external_module_export(declaration_name)
            };
            statements.push(export_statement);
        }
        if statements.len() == 1 {
            if class_alias.is_some() {
                self.pop_enclosing_class();
            }
            return statements[0];
        }
        let result = self.factory_mut().new_syntax_list(statements);
        if class_alias.is_some() {
            self.pop_enclosing_class();
        }
        result
    }

    fn pop_enclosing_class(&mut self) {
        self.enclosing_classes.pop();
    }

    fn push_enclosing_class(&mut self, cls: ast::Node) {
        self.enclosing_classes.push(cls);
    }

    fn referenced_value_declaration(&self, node: ast::Node) -> Option<ast::Node> {
        let original = self.emit_context.most_original(&node);
        self.facts.referenced_value_declaration(original)
    }

    fn has_internal_static_reference(&self, node: ast::Node) -> bool {
        let class_node = self.emit_context.most_original(&node);
        let source = self.store_for(node);
        let Some(members) = source.source_members(node) else {
            return false;
        };
        members
            .iter()
            .any(|member| self.is_or_contains_static_self_reference(member, class_node))
    }

    fn is_or_contains_static_self_reference(&self, node: ast::Node, class_node: ast::Node) -> bool {
        let store = self.store_for(node);
        if ast::is_identifier(store, node)
            && self
                .referenced_value_declaration(node)
                .is_some_and(|declaration| {
                    self.emit_context.most_original(&declaration) == class_node
                })
        {
            return true;
        }
        // For PropertyAccessExpression, only check the expression, not the name.
        // The .Name() is a property access name, not a value reference to the class.
        if ast::is_property_access_expression(store, node) {
            return store.expression(node).is_some_and(|expression| {
                self.is_or_contains_static_self_reference(expression, class_node)
            });
        }
        let mut found = false;
        let _ = store.for_each_present_child(node, |child| {
            if self.is_or_contains_static_self_reference(child, class_node) {
                found = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        found
    }

    /**
     * Gets a local alias for a class declaration if it is a decorated class with an internal
     * reference to the static side of the class. This is necessary to avoid issues with
     * double-binding semantics for the class name.
     */
    fn get_class_alias_if_needed(&mut self, node: ast::Node) -> Option<ast::Node> {
        if !self.has_internal_static_reference(node) {
            return None;
        }
        let name_text = {
            let source = self.store_for(node);
            if let Some(name) = source.name(node)
                && !is_generated_identifier(self.emit_context, &name)
            {
                self.store_for(name).text(name).to_string()
            } else {
                "default".to_string()
            }
        };
        let class_alias = self.emit_factory_mut().new_unique_name(&name_text);
        self.emit_context.add_variable_declaration(class_alias);
        self.class_aliases.insert(node, class_alias);

        Some(class_alias)
    }

    fn finish_class_element(&mut self, updated: ast::Node, original: ast::Node) -> ast::Node {
        if updated != original {
            // While we emit the source map for the node after skipping decorators and modifiers,
            // we need to emit the comments for the original range.
            let original_store = self.store_for(original);
            let comment_range = original_store.loc(original);
            let source_map_range = move_range_past_modifiers(original_store, original);
            self.emit_context.set_comment_range(&updated, comment_range);
            self.emit_context
                .set_source_map_range(&updated, source_map_range);
        }
        updated
    }

    // visitPropertyNameOfClassElement visits the property name of a class element,
    // for use when emitting property initializers. For a computed property on a node
    // with decorators, a temporary value is stored for later use.
    fn visit_property_name_of_class_element(&mut self, member: ast::Node) -> Option<ast::Node> {
        let member_store = self.store_for(member);
        let name = member_store.name(member)?;
        let has_decorators = ast::has_decorators(member_store, member);
        let name_is_computed = ast::is_computed_property_name(self.store_for(name), name);
        if name_is_computed && has_decorators {
            let expression = {
                let name_store = self.store_for(name);
                name_store.expression(name)
            };
            let expression = self.visit_node(expression)?;
            let inner_expression =
                ast::skip_partially_emitted_expressions(self.factory().store(), expression);
            if !is_simple_inlineable_expression(self.factory().store(), inner_expression) {
                let factory_store_id = self.emit_context.factory.node_factory.store().store_id();
                let generated_name = self.emit_context.new_generated_name_for_node(name);
                self.emit_context.add_variable_declaration(generated_name);
                let assignment = self
                    .emit_factory_mut()
                    .new_assignment_expression(generated_name, expression);
                return Some(if name.store_id() == factory_store_id {
                    self.factory_mut()
                        .update_computed_property_name(name, assignment)
                } else {
                    let source_file = self
                        .emit_context
                        .source_file_handle_for_node(name)
                        .expect("emit context should resolve computed property name source file");
                    let source = source_file.store();
                    self.factory_mut()
                        .update_computed_property_name_from_store(source, name, assignment)
                });
            }
        }
        self.visit_node(Some(name))
    }

    /**
     * Generates a __decorate helper call for a class constructor.
     *
     * @param node The class node.
     */
    fn get_constructor_decoration_statement(&mut self, node: ast::Node) -> Option<ast::Node> {
        let expression = self.generate_constructor_decoration_expression(node)?;
        let result = self.factory_mut().new_expression_statement(expression);
        self.emit_context.set_original(&result, &node);
        Some(result)
    }

    /**
     * Generates a __decorate helper call for a class constructor.
     *
     * @param node The class node.
     */
    fn generate_constructor_decoration_expression(&mut self, node: ast::Node) -> Option<ast::Node> {
        let all_decorators = {
            let source = self.store_for(node);
            get_all_decorators_of_class(source, node, true)
        };
        // Decorator expressions are evaluated outside the class body, so references to the
        // class name should use the original binding, not the class alias. In Strada, this is
        // handled by NodeCheckFlags.ConstructorReference which is only set for identifiers
        // inside the class body. Since Corsa lacks per-node flags, we temporarily pop the
        // enclosing class to prevent alias substitution during decorator expression visiting.
        let has_alias = self
            .enclosing_classes
            .last()
            .is_some_and(|class| *class == node);
        if has_alias {
            self.pop_enclosing_class();
        }
        let decorator_expressions = self.transform_all_decorators_of_declaration(all_decorators);
        if has_alias {
            self.push_enclosing_class(node);
        }
        if decorator_expressions.is_empty() {
            return None;
        }

        // When we used to transform to ES5/3 this would be moved inside an IIFE and should reference the name
        // without any block-scoped variable collision handling - but we don't support that anymore, so we always
        // use the local name for the class
        let local_name = self.declaration_name_without_comments(node);
        let decorate = self.emit_factory_mut().new_decorate_helper(
            &decorator_expressions,
            local_name,
            None,
            None,
        );
        let class_alias = self.class_aliases.get(&node).copied();
        let assignment_target = if let Some(class_alias) = class_alias {
            self.emit_factory_mut()
                .new_assignment_expression(class_alias, decorate)
        } else {
            decorate
        };
        let local_name = self.declaration_name_without_comments(node);
        let expression = self
            .emit_factory_mut()
            .new_assignment_expression(local_name, assignment_target);
        self.emit_context
            .set_emit_flags(&expression, printer::EF_NO_COMMENTS);
        let location = {
            let source = self.store_for(node);
            move_range_past_modifiers(source, node)
        };
        self.emit_context
            .set_source_map_range(&expression, location);
        Some(expression)
    }

    fn transform_decorators_of_class_elements(
        &mut self,
        node: ast::Node,
        members: ast::NodeList,
    ) -> (ast::NodeList, Vec<ast::Node>) {
        let mut decoration_statements = Vec::new();
        decoration_statements.extend(self.get_class_element_decoration_statements(node, false));
        decoration_statements.extend(self.get_class_element_decoration_statements(node, true));
        if has_class_element_with_decorator_containing_private_identifier_in_expression(
            self.store_for(node),
            node,
        ) {
            let mut member_list: Vec<_> = members.iter(self.factory().store()).collect();
            let synthesized = core::undefined_text_range();
            let decoration_statement_list = self.factory_mut().new_node_list(
                synthesized,
                synthesized,
                decoration_statements.clone(),
            );
            let block = self
                .factory_mut()
                .new_block(decoration_statement_list, true);
            member_list.push(
                self.factory_mut()
                    .new_class_static_block_declaration(None::<ast::ModifierList>, Some(block)),
            );
            let members = self
                .factory_mut()
                .new_node_list(synthesized, synthesized, member_list);
            decoration_statements.clear();
            return (members, decoration_statements);
        }
        (members, decoration_statements)
    }

    /**
     * Generates statements used to apply decorators to either the static or instance members
     * of a class.
     *
     * @param node The class node.
     * @param isStatic A value indicating whether to generate statements for static or
     *                 instance members.
     */
    fn get_class_element_decoration_statements(
        &mut self,
        node: ast::Node,
        is_static: bool,
    ) -> Vec<ast::Node> {
        self.generate_class_element_decoration_expressions(node, is_static)
            .into_iter()
            .map(|expression| self.factory_mut().new_expression_statement(expression))
            .collect()
    }

    /**
     * Generates expressions used to apply decorators to either the static or instance members
     * of a class.
     *
     * @param node The class node.
     * @param isStatic A value indicating whether to generate expressions for static or
     *                 instance members.
     */
    fn generate_class_element_decoration_expressions(
        &mut self,
        node: ast::Node,
        is_static: bool,
    ) -> Vec<ast::Node> {
        let decorated_elements = {
            let source = self.store_for(node);
            get_decorated_class_elements(source, node, is_static)
        };
        decorated_elements
            .into_iter()
            .filter_map(|member| self.generate_class_element_decoration_expression(node, member))
            .collect()
    }

    /**
     * Generates an expression used to evaluate class element decorators at runtime.
     *
     * @param node The class node that contains the member.
     * @param member The class member.
     */
    fn generate_class_element_decoration_expression(
        &mut self,
        node: ast::Node,
        member: ast::Node,
    ) -> Option<ast::Node> {
        let all_decorators = {
            let source = self.store_for(member);
            get_all_decorators_of_class_element(source, member, node, true)
        };
        let decorator_expressions = self.transform_all_decorators_of_declaration(all_decorators);
        if decorator_expressions.is_empty() {
            return None;
        }

        let local_name = self.declaration_name(node);
        let (is_static, is_property_without_accessor, source_map_range) = {
            let member_store = self.store_for(member);
            (
                ast::is_static(member_store, member),
                ast::is_property_declaration(member_store, member)
                    && !ast::has_accessor_modifier(member_store, member),
                move_range_past_modifiers(member_store, member),
            )
        };
        let target = if is_static {
            local_name
        } else {
            let prototype = self.factory_mut().new_identifier("prototype");
            self.factory_mut().new_property_access_expression(
                local_name,
                None::<ast::Node>,
                prototype,
                ast::NodeFlags::NONE,
            )
        };
        let member_name = self.get_expression_for_property_name(
            member,
            !self
                .store_for(member)
                .flags(member)
                .contains(ast::NodeFlags::AMBIENT),
        );
        let descriptor = if is_property_without_accessor {
            // We emit `void 0` here to indicate to `__decorate` that it can invoke `Object.defineProperty` directly, but that it
            // should not invoke `Object.getOwnPropertyDescriptor`.
            Some(self.emit_factory_mut().new_void_zero_expression())
        } else {
            // We emit `null` here to indicate to `__decorate` that it can invoke `Object.getOwnPropertyDescriptor` directly.
            // We have this extra argument here so that we can inject an explicit property descriptor at a later date.
            Some(
                self.factory_mut()
                    .new_keyword_expression(ast::Kind::NullKeyword),
            )
        };
        let expression = self.emit_factory_mut().new_decorate_helper(
            &decorator_expressions,
            target,
            member_name,
            descriptor,
        );
        self.emit_context
            .set_emit_flags(&expression, printer::EF_NO_COMMENTS);
        self.emit_context
            .set_source_map_range(&expression, source_map_range);
        Some(expression)
    }

    fn get_expression_for_property_name(
        &mut self,
        member: ast::Node,
        generate_name_for_computed_property_name: bool,
    ) -> Option<ast::Node> {
        let member_store = self.store_for(member);
        let name = member_store.name(member)?;
        let name_store = self.store_for(name);
        let name_kind = name_store.kind(name);
        Some(match name_kind {
            ast::Kind::PrivateIdentifier => self.factory_mut().new_identifier(""),
            ast::Kind::ComputedPropertyName => {
                let expression = {
                    let name_store = self.store_for(name);
                    name_store.expression(name)
                };
                let should_generate = generate_name_for_computed_property_name
                    && expression.is_some_and(|expression| {
                        !is_simple_inlineable_expression(self.store_for(expression), expression)
                    });
                if should_generate {
                    self.emit_context.new_generated_name_for_node(name)
                } else {
                    expression?
                }
            }
            ast::Kind::Identifier => {
                let text = {
                    let name_store = self.store_for(name);
                    name_store.text(name).to_string()
                };
                self.factory_mut()
                    .new_string_literal(&text, ast::TokenFlags::NONE)
            }
            _ => {
                if name.store_id() == self.factory().store().store_id() {
                    self.factory_mut().clone_node(name)
                } else {
                    let source = self.source;
                    self.factory_mut().deep_clone_node_from_store(source, name)
                }
            }
        })
    }

    /**
     * Transforms all of the decorators for a declaration into an array of expressions.
     *
     * @param allDecorators An object containing all of the decorators for the declaration.
     */
    fn transform_all_decorators_of_declaration(
        &mut self,
        all_decorators: Option<AllDecorators>,
    ) -> Vec<ast::Node> {
        let Some(all_decorators) = all_decorators else {
            return Vec::new();
        };
        // ensure that metadata decorators are last
        let (metadata, decorators): (Vec<_>, Vec<_>) = all_decorators
            .decorators
            .into_iter()
            .partition(|decorator| self.is_synthetic_metadata_decorator(*decorator));
        let mut decorator_expressions = Vec::new();
        for decorator in decorators {
            if let Some(expression) = self.transform_decorator(decorator) {
                decorator_expressions.push(expression);
            }
        }
        for (parameter_offset, decorators) in all_decorators.parameters.into_iter().enumerate() {
            decorator_expressions.extend(
                self.transform_decorators_of_parameter(decorators, parameter_offset as i32),
            );
        }
        for decorator in metadata {
            if let Some(expression) = self.transform_decorator(decorator) {
                decorator_expressions.push(expression);
            }
        }
        decorator_expressions
    }

    fn is_synthetic_metadata_decorator(&mut self, decorator: ast::Node) -> bool {
        let source = self.store_for(decorator);
        source.expression(decorator).is_some_and(|expression| {
            self.emit_context
                .is_call_to_helper(&expression, "__metadata")
        })
    }

    fn transform_decorator(&mut self, decorator: ast::Node) -> Option<ast::Node> {
        let expression = {
            let source = self.store_for(decorator);
            source.expression(decorator)?
        };
        self.visit_node(Some(expression))
    }

    /**
     * Transforms the decorators of a parameter.
     *
     * @param decorators The decorators for the parameter at the provided offset.
     * @param parameterOffset The offset of the parameter.
     */
    fn transform_decorators_of_parameter(
        &mut self,
        decorators: Vec<ast::Node>,
        parameter_offset: i32,
    ) -> Vec<ast::Node> {
        decorators
            .into_iter()
            .filter_map(|decorator| {
                let decorator_expression = {
                    let source = self.store_for(decorator);
                    source.expression(decorator)?
                };
                let location = {
                    let source = self.store_for(decorator_expression);
                    source.loc(decorator_expression)
                };
                let expression = self.visit_node(Some(decorator_expression))?;
                let helper = self.emit_factory_mut().new_param_helper(
                    expression,
                    parameter_offset,
                    location,
                );
                self.emit_context
                    .set_emit_flags(&helper, printer::EF_NO_COMMENTS);
                Some(helper)
            })
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

    fn lift_to_block_or_empty(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = match node {
            Some(node) => node,
            None => {
                let statements = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    Vec::<ast::Node>::new(),
                );
                return Some(self.factory_mut().new_block(statements, true));
            }
        };
        Some(self.lift_to_block(node))
    }

    fn lift_to_block(&mut self, node: ast::Node) -> ast::Node {
        let store = self.store_for(node);
        let nodes = if store.kind(node) == ast::Kind::SyntaxList {
            store
                .syntax_list_children(node)
                .expect("SyntaxList should have children")
                .iter()
                .flatten()
                .collect::<Vec<_>>()
        } else {
            vec![node]
        };
        let nodes = nodes
            .into_iter()
            .map(|node| self.preserve_node(node))
            .collect::<Vec<_>>();
        if nodes.len() == 1 {
            nodes[0]
        } else {
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
            );
            self.factory_mut().new_block(statements, true)
        }
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for LegacyDecoratorsRuntime<'_, 'source> {
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ast::NodeFactory {
        self.factory()
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        self.factory_mut()
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

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;

        self.emit_context.start_variable_environment();
        for node in source_list.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        let declarations = self.emit_context.end_variable_environment();
        let (visited, environment_changed) = self
            .emit_context
            .merge_environment_for_resolved_nodes(&visited, &declarations);

        if changed || environment_changed {
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

    fn preserved_source_node_list_input_matches(
        &self,
        source: Option<&ast::SourceNodeListInput>,
        output: Option<ast::NodeList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_node_list());
        }
        self.import_state.preserved_source_node_list_input_matches(
            self.source,
            self.factory(),
            Some(source),
            output,
        )
    }

    fn preserved_source_modifier_list_input_matches(
        &self,
        source: Option<&ast::SourceModifierListInput>,
        output: Option<ast::ModifierList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_modifier_list());
        }
        self.import_state
            .preserved_source_modifier_list_input_matches(
                self.source,
                self.factory(),
                Some(source),
                output,
            )
    }

    fn preserved_source_raw_node_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawNodeSliceInput>,
        output: Option<ast::RawNodeSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_raw_node_slice());
        }
        self.import_state
            .preserved_source_raw_node_slice_input_matches(
                self.source,
                self.factory(),
                Some(source),
                output,
            )
    }

    fn preserved_source_raw_string_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawStringSliceInput>,
        output: Option<ast::RawStringSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_raw_string_slice());
        }
        self.import_state
            .preserved_source_raw_string_slice_input_matches(
                self.source,
                self.factory(),
                Some(source),
                output,
            )
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
            let nodes = store
                .syntax_list_children(visited)
                .expect("SyntaxList should have children")
                .iter()
                .collect::<Vec<_>>();
            if nodes.len() != 1 {
                return Some(visited);
            }
            visited = nodes[0]?;
        }
        Some(self.preserve_node(visited))
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let old_flags = self.emit_context.begin_visit_parameters();
        let mut visited = Vec::with_capacity(nodes.len());
        let mut changed = false;
        for node in nodes.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        let (visited, changed) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        if changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                nodes.loc(),
                nodes.range(),
                visited,
                nodes.has_trailing_comma(),
            ))
        } else {
            Some(nodes.as_node_list())
        }
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let updated = self.visit_node(node);
        self.emit_context.finish_visit_function_body(updated)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node?;
        self.emit_context.begin_visit_iteration_body();
        let updated = self.visit_embedded_statement(node);
        self.emit_context.finish_visit_iteration_body(updated)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        match node {
            Some(node) => {
                let visited = self.visit(&node);
                let lifted = self.lift_to_block_or_empty(visited);
                let updated = self
                    .emit_context
                    .finish_visit_embedded_statement(&node, lifted);
                updated.map(|updated| self.preserve_node(updated))
            }
            None => None,
        }
    }
}

struct AllDecorators {
    decorators: Vec<ast::Node>,
    parameters: Vec<Vec<ast::Node>>,
}

fn decorator_contains_private_identifier_in_expression(
    store: &ast::AstStore,
    decorator: ast::Node,
) -> bool {
    store
        .subtree_facts(decorator)
        .contains(ast::SubtreeFacts::CONTAINS_PRIVATE_IDENTIFIER_IN_EXPRESSION)
}

fn parameter_decorators_contain_private_identifier_in_expression(
    store: &ast::AstStore,
    parameter_decorators: &[ast::Node],
) -> bool {
    parameter_decorators
        .iter()
        .any(|decorator| decorator_contains_private_identifier_in_expression(store, *decorator))
}

fn has_class_element_with_decorator_containing_private_identifier_in_expression(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    let Some(members) = store.source_members(node) else {
        return false;
    };
    members.iter().any(|member| {
        if !ast::can_have_decorators(store, member) {
            return false;
        }
        let Some(all_decorators) = get_all_decorators_of_class_element(store, member, node, true)
        else {
            return false;
        };
        all_decorators
            .decorators
            .iter()
            .any(|decorator| decorator_contains_private_identifier_in_expression(store, *decorator))
            || all_decorators.parameters.iter().any(|decorators| {
                parameter_decorators_contain_private_identifier_in_expression(store, decorators)
            })
    })
}

fn get_decorators(store: &ast::AstStore, node: ast::Node) -> Vec<ast::Node> {
    store
        .source_modifiers(node)
        .map(|modifiers| {
            modifiers
                .nodes()
                .iter()
                .filter(|modifier| store.kind(*modifier) == ast::Kind::Decorator)
                .collect()
        })
        .unwrap_or_default()
}

/**
 * Gets an array of arrays of decorators for the parameters of a function-like node.
 * The offset into the result array should correspond to the offset of the parameter.
 *
 * @param node The function-like node.
 */
fn get_decorators_of_parameters(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> Vec<Vec<ast::Node>> {
    let mut decorators = Vec::new();
    if let Some(node) = node {
        let parameters = store
            .parameters(node)
            .map(|parameters| parameters.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let first_parameter_is_this = parameters
            .first()
            .is_some_and(|parameter| ast::is_this_parameter(store, *parameter));
        let first_parameter_offset = if first_parameter_is_this { 1 } else { 0 };
        let num_parameters = parameters.len().saturating_sub(first_parameter_offset);
        for i in 0..num_parameters {
            let parameter = parameters[i + first_parameter_offset];
            let parameter_decorators = get_decorators(store, parameter);
            if !decorators.is_empty() || !parameter_decorators.is_empty() {
                if decorators.is_empty() {
                    decorators = vec![Vec::new(); num_parameters];
                }
                decorators[i] = parameter_decorators;
            }
        }
    }
    decorators
}

/**
 * Gets an allDecorators object containing the decorators for the class and the decorators for the
 * parameters of the constructor of the class.
 *
 * @param node The class node.
 *
 * @internal
 */
fn get_all_decorators_of_class(
    store: &ast::AstStore,
    node: ast::Node,
    use_legacy_decorators: bool,
) -> Option<AllDecorators> {
    let decorators = get_decorators(store, node);
    let parameters = if use_legacy_decorators {
        get_decorators_of_parameters(store, ast::get_first_constructor_with_body(store, node))
    } else {
        Vec::new()
    };
    if decorators.is_empty() && parameters.is_empty() {
        return None;
    }
    Some(AllDecorators {
        decorators,
        parameters,
    })
}

/**
 * Gets an allDecorators object containing the decorators for the member and its parameters.
 *
 * @param parent The class node that contains the member.
 * @param member The class member.
 *
 * @internal
 */
fn get_all_decorators_of_class_element(
    store: &ast::AstStore,
    member: ast::Node,
    parent: ast::Node,
    use_legacy_decorators: bool,
) -> Option<AllDecorators> {
    match store.kind(member) {
        ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
            if !use_legacy_decorators {
                return get_all_decorators_of_method(store, member, false);
            }
            get_all_decorators_of_accessors(store, member, parent, true)
        }
        ast::Kind::MethodDeclaration => {
            get_all_decorators_of_method(store, member, use_legacy_decorators)
        }
        ast::Kind::PropertyDeclaration => get_all_decorators_of_property(store, member),
        _ => None,
    }
}

fn get_all_decorators_of_property(
    store: &ast::AstStore,
    property: ast::Node,
) -> Option<AllDecorators> {
    let decorators = get_decorators(store, property);
    if decorators.is_empty() {
        return None;
    }
    Some(AllDecorators {
        decorators,
        parameters: Vec::new(),
    })
}

fn get_all_decorators_of_method(
    store: &ast::AstStore,
    method: ast::Node,
    use_legacy_decorators: bool,
) -> Option<AllDecorators> {
    store.body(method)?;
    let decorators = get_decorators(store, method);
    let parameters = if use_legacy_decorators {
        get_decorators_of_parameters(store, Some(method))
    } else {
        Vec::new()
    };
    if decorators.is_empty() && parameters.is_empty() {
        return None;
    }
    Some(AllDecorators {
        decorators,
        parameters,
    })
}

/**
 * Gets an allDecorators object containing the decorators for the accessor and its parameters.
 *
 * @param parent The class node that contains the accessor.
 * @param accessor The class accessor member.
 */
fn get_all_decorators_of_accessors(
    store: &ast::AstStore,
    accessor: ast::Node,
    parent: ast::Node,
    use_legacy_decorators: bool,
) -> Option<AllDecorators> {
    store.body(accessor)?;
    let members = store.members(parent)?;
    let member_nodes = members.iter().collect::<Vec<_>>();
    let decls = ast::get_all_accessor_declarations(store, &member_nodes, accessor);
    let first_accessor_with_decorators = if ast::has_decorators(store, decls.first_accessor) {
        Some(decls.first_accessor)
    } else if decls
        .second_accessor
        .is_some_and(|second_accessor| ast::has_decorators(store, second_accessor))
    {
        decls.second_accessor
    } else {
        None
    };

    if first_accessor_with_decorators.is_none() || Some(accessor) != first_accessor_with_decorators
    {
        return None;
    }

    let decorators = get_decorators(store, first_accessor_with_decorators.unwrap());
    let parameters = if use_legacy_decorators && let Some(set_accessor) = decls.set_accessor {
        get_decorators_of_parameters(store, Some(set_accessor))
    } else {
        Vec::new()
    };

    if decorators.is_empty() && parameters.is_empty() {
        return None;
    }

    Some(AllDecorators {
        decorators,
        parameters,
    })
}

/**
 * Determines whether a class member is either a static or an instance member of a class
 * that is decorated, or has parameters that are decorated.
 *
 * @param member The class member.
 */
fn is_decorated_class_element(
    store: &ast::AstStore,
    member: ast::Node,
    is_static_element: bool,
    parent: ast::Node,
) -> bool {
    is_static_element == ast::is_static(store, member)
        && ast::class_element_or_class_element_parameter_is_decorated(store, true, member, parent)
        && get_all_decorators_of_class_element(store, member, parent, true).is_some()
}

/**
 * Gets either the static or instance members of a class that are decorated, or have
 * parameters that are decorated.
 *
 * @param node The class containing the member.
 * @param isStatic A value indicating whether to retrieve static or instance members of
 *                 the class.
 */
fn get_decorated_class_elements(
    store: &ast::AstStore,
    node: ast::Node,
    is_static: bool,
) -> Vec<ast::Node> {
    store
        .members(node)
        .map(|members| {
            members
                .iter()
                .filter(|member| is_decorated_class_element(store, *member, is_static, node))
                .collect()
        })
        .unwrap_or_default()
}

// MoveRangePastModifiers returns a text range that starts past any modifiers on the node.
fn move_range_past_modifiers(store: &ast::AstStore, node: ast::Node) -> core::TextRange {
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
fn move_range_past_decorators(store: &ast::AstStore, node: ast::Node) -> core::TextRange {
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

fn is_export_or_default_or_decorator(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::Decorator
        || store.kind(node) == ast::Kind::ExportKeyword
        || store.kind(node) == ast::Kind::DefaultKeyword
}

fn is_class_static_block_declaration_or_static_property(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    ast::is_class_static_block_declaration(store, node)
        || (ast::is_property_declaration(store, node) && ast::has_static_modifier(store, node))
}

/**
 * A simple inlinable expression is an expression which can be copied into multiple locations
 * without risk of repeating any sideeffects and whose value could not possibly change between
 * any such locations
 */
fn is_simple_inlineable_expression(store: &ast::AstStore, expression: ast::Node) -> bool {
    crate::moduletransforms::utilities::is_simple_inlineable_expression(
        store.kind(expression),
        ast::is_identifier(store, expression),
    )
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for LegacyDecoratorsRuntime<'_, 'source> {}
