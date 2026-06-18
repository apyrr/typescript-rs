use std::collections::HashSet;

use ts_ast as ast;
use ts_printer as printer;

#[derive(Clone, Default)]
pub struct ImportElisionResolverFacts {
    referenced_alias_declarations: HashSet<ast::Node>,
    value_alias_declarations: HashSet<ast::Node>,
    top_level_value_import_equals_with_entity_name: HashSet<ast::Node>,
}

pub fn collect_import_elision_resolver_facts(
    source_file: &ast::SourceFile,
    resolver: &mut dyn printer::EmitResolver,
) -> ImportElisionResolverFacts {
    let mut facts = ImportElisionResolverFacts::default();
    let store = source_file.store();
    let mut stack = vec![source_file.root()];
    while let Some(node) = stack.pop() {
        match store.kind(node) {
            ast::Kind::ImportEqualsDeclaration
            | ast::Kind::ImportClause
            | ast::Kind::NamespaceImport
            | ast::Kind::ImportSpecifier => {
                if resolver.is_referenced_alias_declaration(node) {
                    facts.referenced_alias_declarations.insert(node);
                }
                if store.kind(node) == ast::Kind::ImportEqualsDeclaration
                    && resolver.is_top_level_value_import_equals_with_entity_name(node)
                {
                    facts
                        .top_level_value_import_equals_with_entity_name
                        .insert(node);
                }
            }
            ast::Kind::ExportAssignment | ast::Kind::ExportSpecifier => {
                if resolver.is_value_alias_declaration(node) {
                    facts.value_alias_declarations.insert(node);
                }
            }
            _ => {}
        }
        let _ = store.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }
    facts
}

pub fn visit_source_file_output(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    facts: &ImportElisionResolverFacts,
) -> Option<ast::Node> {
    let current_source_file_is_external_module = ast::AstTraversalState::store_for(
        source_file.store(),
        &emit_context.factory.node_factory,
        root,
    )
    .as_source_file(root)
    .external_module_indicator()
    .is_some();
    let mut tx = ImportElisionTransform {
        source: source_file.store(),
        root,
        emit_context,
        facts,
        current_source_file_is_external_module,
        import_state: ast::AstImportState::new(),
    };
    tx.visit_source_file()
}

struct ImportElisionTransform<'a, 'ec> {
    source: &'a ast::AstStore,
    root: ast::Node,
    emit_context: &'ec mut printer::EmitContext,
    facts: &'a ImportElisionResolverFacts,
    current_source_file_is_external_module: bool,
    import_state: ast::AstImportState,
}

impl ImportElisionTransform<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn is_factory_node(&self, node: ast::Node) -> bool {
        node.store_id() == self.factory().store().store_id()
    }

    fn parse_node(&self, node: ast::Node) -> Option<ast::Node> {
        self.emit_context.parse_node(&node)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if self.is_factory_node(node) {
            return node;
        }
        assert_eq!(
            node.store_id(),
            self.source.store_id(),
            "ImportElision can only preserve nodes from the source file or active factory"
        );
        let imported = self.import_state.preserve_node(
            self.source,
            &mut self.emit_context.factory.node_factory,
            node,
        );
        self.emit_context.set_original(&imported, &node);
        imported
    }

    fn preserve_optional_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.preserve_node(node))
    }

    fn preserve_optional_modifier_list_input(
        &mut self,
        modifiers: Option<&ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        modifiers.map(|modifiers| {
            if modifiers.store_id() == self.factory().store().store_id() {
                modifiers.as_modifier_list()
            } else {
                assert_eq!(
                    modifiers.store_id(),
                    self.source.store_id(),
                    "ImportElision can only preserve modifier lists from the source file or active factory"
                );
                self.import_state.preserve_source_modifier_list_input(
                    self.source,
                    &mut self.emit_context.factory.node_factory,
                    modifiers,
                )
            }
        })
    }

    fn new_node_list_like(
        &mut self,
        source_list: &ast::SourceNodeListInput,
        nodes: Vec<ast::Node>,
    ) -> ast::NodeList {
        self.emit_context
            .factory
            .node_factory
            .new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                nodes,
                source_list.has_trailing_comma(),
            )
    }

    fn preserve_node_list_input(&mut self, list: &ast::SourceNodeListInput) -> ast::NodeList {
        if list.store_id() == self.factory().store().store_id() {
            return list.as_node_list();
        }
        assert_eq!(
            list.store_id(),
            self.source.store_id(),
            "ImportElision can only preserve node lists from the source file or active factory"
        );
        self.import_state.preserve_source_node_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            list,
        )
    }

    fn should_emit_alias_declaration(&self, node: ast::Node) -> bool {
        ast::is_in_js_file(self.emit_context.store_for_node(node), node)
            || self
                .parse_node(node)
                .is_none_or(|node| self.facts.referenced_alias_declarations.contains(&node))
    }

    fn is_value_alias_declaration(&self, node: ast::Node) -> bool {
        self.parse_node(node)
            .is_none_or(|node| self.facts.value_alias_declarations.contains(&node))
    }

    fn should_emit_import_equals_declaration(&self, node: ast::Node) -> bool {
        // preserve old compiler's behavior: emit import declaration (even if we do not consider them referenced) when
        // - current file is not external module
        // - import declaration is top level and target is value imported by entity name
        self.should_emit_alias_declaration(node)
            || (!self.current_source_file_is_external_module
                && self.parse_node(node).is_some_and(|node| {
                    self.facts
                        .top_level_value_import_equals_with_entity_name
                        .contains(&node)
                }))
    }

    fn visit_source_file(&mut self) -> Option<ast::Node> {
        let root = self.root;
        let (statements_input, end_of_file_token) = {
            let source = self.store_for(root);
            (
                ast::SourceNodeListInput::from_source(
                    source
                        .source_statements(root)
                        .expect("source file should have statements"),
                ),
                source.as_source_file(root).end_of_file_token(),
            )
        };
        let statements = self.visit_node_list(statements_input)?;
        let end_of_file_token = self.preserve_optional_node(end_of_file_token);
        Some(if self.is_factory_node(root) {
            self.factory_mut().update_source_file_in_current_store(
                root,
                statements,
                end_of_file_token,
            )
        } else {
            let source_data = self.source.as_source_file(root);
            self.emit_context
                .factory
                .node_factory
                .update_source_file_from_store(
                    self.source,
                    root,
                    &source_data,
                    statements,
                    end_of_file_token,
                )
        })
    }

    fn visit_node_list(&mut self, source_list: ast::SourceNodeListInput) -> Option<ast::NodeList> {
        let mut changed = false;
        let mut visited = Vec::with_capacity(source_list.len());
        for node in source_list.iter() {
            match self.visit_node(node) {
                Some(output) => {
                    changed |= output != node;
                    visited.push(output);
                }
                None => changed = true,
            }
        }
        if !changed {
            return Some(self.preserve_node_list_input(&source_list));
        }
        Some(self.new_node_list_like(&source_list, visited))
    }

    fn visit_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        match self.store_for(node).kind(node) {
            ast::Kind::ImportEqualsDeclaration => self.visit_import_equals_declaration(node),
            ast::Kind::ImportDeclaration => self.visit_import_declaration(node),
            ast::Kind::ImportClause => self.visit_import_clause(node),
            ast::Kind::NamespaceImport => {
                if !self.should_emit_alias_declaration(node) {
                    // elide unused imports
                    return None;
                }
                Some(self.preserve_node(node))
            }
            ast::Kind::NamedImports => self.visit_named_imports(node),
            ast::Kind::ImportSpecifier => {
                if !self.should_emit_alias_declaration(node) {
                    // elide type-only or unused imports
                    return None;
                }
                Some(self.preserve_node(node))
            }
            ast::Kind::ExportAssignment => {
                if !self.is_value_alias_declaration(node) {
                    // elide unused import
                    return None;
                }
                Some(self.preserve_node(node))
            }
            ast::Kind::ExportDeclaration => self.visit_export_declaration(node),
            ast::Kind::NamedExports => self.visit_named_exports(node),
            ast::Kind::ExportSpecifier => {
                if !self.is_value_alias_declaration(node) {
                    // elide unused export
                    return None;
                }
                Some(self.preserve_node(node))
            }
            ast::Kind::ModuleDeclaration => self.visit_module_declaration(node),
            ast::Kind::ModuleBlock => self.visit_module_block(node),
            _ => Some(self.preserve_node(node)),
        }
    }

    fn visit_import_equals_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let is_external_module_import_equals_declaration = {
            let source = self.store_for(node);
            ast::is_external_module_import_equals_declaration(source, node)
        };
        let should_emit = if is_external_module_import_equals_declaration {
            self.should_emit_alias_declaration(node)
        } else {
            self.should_emit_import_equals_declaration(node)
        };
        should_emit.then(|| self.preserve_node(node))
    }

    fn visit_import_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (import_clause, modifiers, module_specifier, attributes) = {
            let source = self.store_for(node);
            (
                source.import_clause(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source.module_specifier(node),
                source.attributes(node),
            )
        };
        // Do not elide a side-effect only import declaration.
        //  import "foo";
        let Some(import_clause) = import_clause else {
            return Some(self.preserve_node(node));
        };
        let import_clause = self.visit_node(import_clause)?;
        let modifiers = self.preserve_optional_modifier_list_input(modifiers.as_ref());
        let module_specifier = self.preserve_optional_node(module_specifier);
        let attributes = attributes.and_then(|attributes| self.visit_node(attributes));
        Some(if self.is_factory_node(node) {
            self.factory_mut().update_import_declaration(
                node,
                modifiers,
                import_clause,
                module_specifier,
                attributes,
            )
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_import_declaration_from_store(
                    self.source,
                    node,
                    modifiers,
                    import_clause,
                    module_specifier,
                    attributes,
                )
        })
    }

    fn visit_import_clause(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (name, named_bindings, phase_modifier) = {
            let source = self.store_for(node);
            (
                source.name(node),
                source.named_bindings(node),
                source.phase_modifier(node),
            )
        };
        let name = self
            .should_emit_alias_declaration(node)
            .then_some(name)
            .flatten()
            .map(|name| self.preserve_node(name));
        let named_bindings =
            named_bindings.and_then(|named_bindings| self.visit_node(named_bindings));
        if name.is_none() && named_bindings.is_none() {
            // all import bindings were elided
            return None;
        }
        Some(if self.is_factory_node(node) {
            self.factory_mut()
                .update_import_clause(node, phase_modifier, name, named_bindings)
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_import_clause_from_store(
                    self.source,
                    node,
                    phase_modifier,
                    name,
                    named_bindings,
                )
        })
    }

    fn visit_named_imports(&mut self, node: ast::Node) -> Option<ast::Node> {
        let elements = {
            let source = self.store_for(node);
            ast::SourceNodeListInput::from_source(source.source_elements(node)?)
        };
        let elements = self.visit_node_list(elements)?;
        if self
            .emit_context
            .factory
            .node_factory
            .emit_node_list_nodes(elements)
            .is_empty()
        {
            // all import specifiers were elided
            return None;
        }
        Some(if self.is_factory_node(node) {
            self.factory_mut().update_named_imports(node, elements)
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_named_imports_from_store(self.source, node, elements)
        })
    }

    fn visit_export_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (export_clause, module_specifier, attributes) = {
            let source = self.store_for(node);
            (
                source.export_clause(node),
                source.module_specifier(node),
                source.attributes(node),
            )
        };
        let export_clause = match export_clause {
            Some(export_clause) => match self.visit_node(export_clause) {
                Some(export_clause) => Some(export_clause),
                None => {
                    // all export bindings were elided
                    return None;
                }
            },
            None => None,
        };
        let module_specifier = self.preserve_optional_node(module_specifier);
        let attributes = attributes.and_then(|attributes| self.visit_node(attributes));
        Some(if self.is_factory_node(node) {
            self.factory_mut().update_export_declaration(
                node,
                None,
                false,
                export_clause,
                module_specifier,
                attributes,
            )
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_export_declaration_from_store(
                    self.source,
                    node,
                    None,
                    false,
                    export_clause,
                    module_specifier,
                    attributes,
                )
        })
    }

    fn visit_named_exports(&mut self, node: ast::Node) -> Option<ast::Node> {
        let elements = {
            let source = self.store_for(node);
            ast::SourceNodeListInput::from_source(source.source_elements(node)?)
        };
        let elements = self.visit_node_list(elements)?;
        if self
            .emit_context
            .factory
            .node_factory
            .emit_node_list_nodes(elements)
            .is_empty()
        {
            // all export specifiers were elided
            return None;
        }
        Some(if self.is_factory_node(node) {
            self.factory_mut().update_named_exports(node, elements)
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_named_exports_from_store(self.source, node, elements)
        })
    }

    fn visit_module_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (body, modifiers, keyword, name) = {
            let source = self.store_for(node);
            (
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source.keyword(node).unwrap_or(ast::Kind::ModuleKeyword),
                source.name(node),
            )
        };
        let body = body.and_then(|body| self.visit_node(body));
        let modifiers = self.preserve_optional_modifier_list_input(modifiers.as_ref());
        let name = self.preserve_optional_node(name);
        Some(if self.is_factory_node(node) {
            self.factory_mut()
                .update_module_declaration(node, modifiers, keyword, name, body)
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_module_declaration_from_store(
                    self.source,
                    node,
                    modifiers,
                    keyword,
                    name,
                    body,
                )
        })
    }

    fn visit_module_block(&mut self, node: ast::Node) -> Option<ast::Node> {
        let statements = {
            let source = self.store_for(node);
            ast::SourceNodeListInput::from_source(source.source_statements(node)?)
        };
        let statements = self.visit_node_list(statements)?;
        Some(if self.is_factory_node(node) {
            self.factory_mut().update_module_block(node, statements)
        } else {
            self.emit_context
                .factory
                .node_factory
                .update_module_block_from_store(self.source, node, statements)
        })
    }
}
