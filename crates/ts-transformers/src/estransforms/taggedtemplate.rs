use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_printer as printer;
use ts_scanner as scanner;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaggedTemplateAction {
    Keep,
    VisitChildren,
    TransformSourceFile,
    ProcessTaggedTemplateExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateCookedValue {
    String(String),
    VoidZero,
}

pub fn tagged_template_action_for_kind(
    kind: ast::Kind,
    subtree_contains_invalid_template_escape: bool,
) -> TaggedTemplateAction {
    if !subtree_contains_invalid_template_escape {
        return TaggedTemplateAction::Keep;
    }

    match kind {
        ast::Kind::SourceFile => TaggedTemplateAction::TransformSourceFile,
        ast::Kind::TaggedTemplateExpression => {
            TaggedTemplateAction::ProcessTaggedTemplateExpression
        }
        _ => TaggedTemplateAction::VisitChildren,
    }
}

pub fn create_template_cooked(text: &str, is_invalid: bool) -> TemplateCookedValue {
    if is_invalid {
        TemplateCookedValue::VoidZero
    } else {
        TemplateCookedValue::String(text.to_owned())
    }
}

pub fn normalize_template_raw_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn trim_raw_template_source(source_text: &str, is_last_piece: bool) -> &str {
    let end_len = if is_last_piece { 1 } else { 2 };
    &source_text[1..source_text.len() - end_len]
}

pub fn should_cache_template_object(is_external_module: bool) -> bool {
    is_external_module
}

pub fn template_object_temp_name() -> &'static str {
    "templateObject"
}
use ts_ast as ast;

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    let mut runtime = TaggedTemplateTransformer {
        source_file,
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        tagged_template_string_declarations: Vec::new(),
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

struct TaggedTemplateTransformer<'ctx, 'source> {
    source_file: &'source ast::SourceFile,
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    tagged_template_string_declarations: Vec<ast::Node>,
}

impl TaggedTemplateTransformer<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        if !source
            .subtree_facts(*node)
            .intersects(ast::SubtreeFacts::CONTAINS_INVALID_TEMPLATE_ESCAPE)
        {
            return Some(*node);
        }
        match source.kind(*node) {
            ast::Kind::SourceFile => Some(self.visit_source_file(*node)),
            ast::Kind::TaggedTemplateExpression => {
                Some(self.visit_tagged_template_expression(*node))
            }
            _ => Some(self.generated_visit_each_child(node)),
        }
    }

    fn visit_source_file(&mut self, node: ast::Node) -> ast::Node {
        self.tagged_template_string_declarations.clear();
        let (
            source_statements_input,
            source_statement_loc,
            source_statement_range,
            source_end_of_file_token,
        ) = {
            let source = self.store_for(node);
            let source_statements = source
                .source_statements(node)
                .expect("source file should have statements");
            (
                ast::SourceNodeListInput::from_source(source_statements),
                source_statements.loc(),
                source_statements.range(),
                source.as_source_file(node).end_of_file_token(),
            )
        };
        let mut statements = self
            .visit_top_level_statements_input(Some(source_statements_input.clone()))
            .expect("source file statements cannot be removed");
        let end_of_file_token = self.visit_token(source_end_of_file_token);

        if !self.tagged_template_string_declarations.is_empty() {
            let mut updated_statements: Vec<ast::Node> =
                statements.iter(self.factory().store()).collect();
            let declarations = self
                .emit_context
                .factory
                .new_node_list(self.tagged_template_string_declarations.clone());
            let declaration_list = self
                .factory_mut()
                .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
            let variable_statement = self
                .factory_mut()
                .new_variable_statement(None::<ast::ModifierList>, declaration_list);
            updated_statements.push(variable_statement);
            statements = self.factory_mut().new_node_list(
                source_statement_loc,
                source_statement_range,
                updated_statements,
            );
        }

        let source_unchanged = self.tagged_template_string_declarations.is_empty()
            && self.preserved_source_node_list_input_matches(
                Some(&source_statements_input),
                Some(statements),
            )
            && self.preserved_source_node_matches(source_end_of_file_token, end_of_file_token);
        let visited = self.update_source_file_from_visited(
            node,
            statements,
            end_of_file_token,
            source_unchanged,
        );

        self.emit_context.add_requested_emit_helpers(&visited);
        visited
    }

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: ast::NodeList,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            if source_unchanged {
                return node;
            }
            return self.factory_mut().update_source_file_in_current_store(
                node,
                statements,
                end_of_file_token,
            );
        }
        if source_unchanged {
            let imported = self.preserve_node(node);
            return self.record_preserved_node(node, imported);
        }
        self.import_state.update_source_file_from_store(
            self.source,
            &mut self.emit_context.factory.node_factory,
            node,
            statements,
            end_of_file_token,
        )
    }

    fn visit_tagged_template_expression(&mut self, node: ast::Node) -> ast::Node {
        self.process_tagged_template_expression(node)
    }

    fn process_tagged_template_expression(&mut self, node: ast::Node) -> ast::Node {
        let (tag, template, node_loc) = {
            let source = self.store_for(node);
            (
                source.tag(node),
                source
                    .template(node)
                    .expect("tagged template expression should have a template"),
                source.loc(node),
            )
        };
        let tag = self
            .visit_node(tag)
            .expect("tagged template expression should have a tag");

        if !self.has_invalid_escape(template) {
            return self.generated_visit_each_child(&node);
        }

        // Build up the template arguments and the raw and cooked strings for the template.
        let mut template_arguments: Vec<ast::Node> = Vec::new(); // placeholder for the template object
        let mut cooked_strings = Vec::new();
        let mut raw_strings = Vec::new();
        template_arguments.push(self.factory_mut().new_token(ast::Kind::Unknown));

        if self.store_for(template).kind(template) == ast::Kind::NoSubstitutionTemplateLiteral {
            cooked_strings.push(self.create_template_cooked(template));
            raw_strings.push(self.get_raw_literal(template));
        } else {
            let (head, spans) = {
                let source = self.store_for(template);
                (
                    source
                        .head(template)
                        .expect("template expression should have a head"),
                    source
                        .source_template_spans(template)
                        .expect("template expression should have spans")
                        .iter()
                        .collect::<Vec<_>>(),
                )
            };
            cooked_strings.push(self.create_template_cooked(head));
            raw_strings.push(self.get_raw_literal(head));
            for span in spans {
                let (literal, expression) = {
                    let source = self.store_for(span);
                    (
                        source
                            .literal(span)
                            .expect("template span should have a literal"),
                        source
                            .expression(span)
                            .expect("template span should have an expression"),
                    )
                };
                cooked_strings.push(self.create_template_cooked(literal));
                raw_strings.push(self.get_raw_literal(literal));
                template_arguments.push(
                    self.visit_node(Some(expression))
                        .expect("template span expression should be visited"),
                );
            }
        }

        let cooked_array_list = self.emit_context.factory.new_node_list(cooked_strings);
        let cooked_array = self
            .factory_mut()
            .new_array_literal_expression(cooked_array_list, false);
        let raw_array_list = self.emit_context.factory.new_node_list(raw_strings);
        let raw_array = self
            .factory_mut()
            .new_array_literal_expression(raw_array_list, false);
        let helper_call = self
            .emit_context
            .factory
            .new_template_object_helper(cooked_array, raw_array);

        // Create a variable to cache the template object if we're in a module.
        // Do not do this in the global scope, as any variable we currently generate could conflict with
        // variables from outside of the current compilation. In the future, we can revisit this behavior.
        if ast::is_external_module(self.source_file) {
            let temp_var = self
                .emit_context
                .factory
                .new_unique_name(template_object_temp_name());
            let declaration = self
                .factory_mut()
                .new_variable_declaration(temp_var, None, None, None);
            self.tagged_template_string_declarations.push(declaration);
            let assignment = self
                .emit_context
                .factory
                .new_assignment_expression(temp_var, helper_call);
            template_arguments[0] = self
                .emit_context
                .factory
                .new_logical_or_expression(temp_var, assignment);
        } else {
            template_arguments[0] = helper_call;
        }

        let arguments = self.emit_context.factory.new_node_list(template_arguments);
        let call = self.factory_mut().new_call_expression(
            tag,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        );
        self.factory_mut().place_emit_synthetic_node(call, node_loc);
        call
    }

    fn create_template_cooked(&mut self, template: ast::Node) -> ast::Node {
        let is_invalid = self
            .store_for(template)
            .template_flags(template)
            .is_some_and(|flags| flags.intersects(ast::TokenFlags::IS_INVALID));
        if is_invalid {
            return self.emit_context.factory.new_void_zero_expression();
        }
        let text = self.store_for(template).text(template);
        self.factory_mut()
            .new_string_literal(text, ast::TokenFlags::NONE)
    }

    fn get_raw_literal(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let mut text = source.raw_text(node).unwrap_or_default();
        let loc = source.loc(node);
        if text.is_empty() {
            let source_node = self.emit_context.most_original(&node);
            let source_file = self
                .emit_context
                .source_file_handle_for_node(source_node)
                .expect("tagged template literal should have a source file");
            text = scanner::get_source_text_of_node_from_source_file(
                &source_file,
                &source_node,
                false, /*include_trivia*/
            );
            // text contains the original source, it will also contain quotes ("`"), dollar signs and braces ("${" and "}"),
            // thus we need to remove those characters.
            // First template piece starts with "`", others with "}"
            // Last template piece ends with "`", others with "${"
            let is_last = matches!(
                source_file.store().kind(source_node),
                ast::Kind::NoSubstitutionTemplateLiteral | ast::Kind::TemplateTail
            );
            text = trim_raw_template_source(&text, is_last).to_owned();
        }

        // Newline normalization:
        // ES6 Spec 11.8.6.1 - Static Semantics of TV's and TRV's
        // <CR><LF> and <CR> LineTerminatorSequences are normalized to <LF> for both TV and TRV.
        text = normalize_template_raw_text(&text);

        let result = self
            .factory_mut()
            .new_string_literal(text, ast::TokenFlags::NONE);
        self.factory_mut().place_emit_synthetic_node(result, loc);
        result
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

    fn has_invalid_escape(&self, template: ast::Node) -> bool {
        let source = self.store_for(template);
        if source.kind(template) == ast::Kind::NoSubstitutionTemplateLiteral {
            return source
                .template_flags(template)
                .is_some_and(|flags| flags.intersects(ast::TokenFlags::CONTAINS_INVALID_ESCAPE));
        }
        let head = source
            .head(template)
            .expect("template expression should have a head");
        if source
            .template_flags(head)
            .is_some_and(|flags| flags.intersects(ast::TokenFlags::CONTAINS_INVALID_ESCAPE))
        {
            return true;
        }
        for span in source
            .source_template_spans(template)
            .expect("template expression should have spans")
            .iter()
        {
            let literal = source
                .literal(span)
                .expect("template span should have a literal");
            if source
                .template_flags(literal)
                .is_some_and(|flags| flags.intersects(ast::TokenFlags::CONTAINS_INVALID_ESCAPE))
            {
                return true;
            }
        }
        false
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for TaggedTemplateTransformer<'_, 'source> {
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for TaggedTemplateTransformer<'_, 'source> {}
