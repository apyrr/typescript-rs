pub mod jsx;

use std::collections::HashMap;

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;
use ts_printer::{AutoGenerateOptions, GeneratedIdentifierFlags};
use ts_scanner as scanner;

use crate::{SourceFileTransformer, TransformOptions, Transformer};

pub fn new_jsx_transformer(opts: &TransformOptions) -> Transformer {
    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        SourceFileTransformer::Jsx {
            compiler_options: opts.compiler_options.clone(),
            facts: opts.jsx_facts.clone().unwrap_or_default(),
        },
        Some(opts.context.fork()),
    );
    tx
}

#[derive(Clone, Default)]
pub struct JsxResolverFacts {
    factory_entity: Option<String>,
    fragment_factory_entity: Option<String>,
    referenced_export_containers: HashMap<(core::TextRange, String), ast::Node>,
}

pub fn collect_jsx_resolver_facts(
    source_file: &ast::SourceFile,
    resolver: &mut dyn printer::EmitResolver,
    compiler_options: &core::CompilerOptions,
) -> JsxResolverFacts {
    let factory_entity = resolver.get_jsx_factory_entity_text(source_file.root());
    let fragment_factory_entity = resolver.get_jsx_fragment_factory_entity_text(source_file.root());
    let mut react_namespaces = Vec::new();
    add_unique_jsx_namespace(
        &mut react_namespaces,
        jsx_factory_namespace(factory_entity.as_deref()),
    );
    add_unique_jsx_namespace(
        &mut react_namespaces,
        jsx_factory_namespace(fragment_factory_entity.as_deref()),
    );
    add_unique_jsx_namespace(
        &mut react_namespaces,
        jsx_react_namespace(&compiler_options.react_namespace),
    );

    let mut referenced_export_containers = HashMap::new();
    let mut stack = vec![source_file.root()];
    while let Some(node) = stack.pop() {
        let store = source_file.store();
        if matches!(
            store.kind(node),
            ast::Kind::JsxOpeningElement
                | ast::Kind::JsxSelfClosingElement
                | ast::Kind::JsxOpeningFragment
        ) {
            for react_namespace in react_namespaces.iter() {
                if let Some(container) = resolver
                    .get_referenced_export_container_for_identifier_text(
                        node,
                        react_namespace,
                        false,
                    )
                {
                    referenced_export_containers
                        .insert((store.loc(node), react_namespace.clone()), container);
                }
            }
        }
        let _ = store.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }

    JsxResolverFacts {
        factory_entity,
        fragment_factory_entity,
        referenced_export_containers,
    }
}

fn jsx_react_namespace(react_namespace: &str) -> String {
    if react_namespace.is_empty() {
        "React".to_string()
    } else {
        react_namespace.to_string()
    }
}

fn jsx_factory_namespace(entity: Option<&str>) -> String {
    entity
        .and_then(|entity| entity.split('.').next())
        .filter(|namespace| !namespace.is_empty())
        .unwrap_or("React")
        .to_string()
}

fn add_unique_jsx_namespace(react_namespaces: &mut Vec<String>, react_namespace: String) {
    if !react_namespaces
        .iter()
        .any(|existing| existing == &react_namespace)
    {
        react_namespaces.push(react_namespace);
    }
}

#[derive(Clone, Copy)]
struct UtilizedImplicitRuntimeImport {
    import_specifier: ast::Node,
    name: ast::Node,
}

#[derive(Clone)]
struct UtilizedImplicitRuntimeImportEntry {
    import_source: String,
    imports: Vec<UtilizedImplicitRuntimeImport>,
}

pub(crate) fn visit_jsx_source_file_root(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    resolver_facts: JsxResolverFacts,
) -> Option<ast::Node> {
    let source =
        ast::AstTraversalState::store_for(file.store(), &emit_context.factory.node_factory, root);
    let facts = jsx::JsxFacts {
        subtree_contains_jsx: source
            .subtree_facts(root)
            .contains(ast::SubtreeFacts::CONTAINS_JSX)
            || source
                .parser_access()
                .source_file_statement_list(root)
                .iter()
                .any(|node| {
                    contains_jsx_node(file.store(), &emit_context.factory.node_factory, node)
                }),
        is_declaration_file: source.as_source_file(root).is_declaration_file(),
        ..Default::default()
    };

    match jsx::jsx_action_for_kind(ast::Kind::SourceFile, facts) {
        jsx::JsxAction::Keep | jsx::JsxAction::SkipSourceFile => None,
        jsx::JsxAction::TransformSourceFile => {
            let mut runtime = JsxTransformerRuntime {
                file,
                source: file.store(),
                emit_context,
                import_state: ast::AstImportState::new(),
                compiler_options,
                facts: resolver_facts,
                import_specifier: String::new(),
                utilized_implicit_runtime_imports: Vec::new(),
                in_jsx_child: false,
                filename_declaration: None,
            };
            runtime.visit_node(Some(root))
        }
        _ => None,
    }
}

fn contains_jsx_node(source: &ast::AstStore, factory: &ast::NodeFactory, node: ast::Node) -> bool {
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        let store = ast::AstTraversalState::store_for(source, factory, node);
        if store
            .subtree_facts(node)
            .contains(ast::SubtreeFacts::CONTAINS_JSX)
            || matches!(
                store.kind(node),
                ast::Kind::JsxElement
                    | ast::Kind::JsxSelfClosingElement
                    | ast::Kind::JsxFragment
                    | ast::Kind::JsxText
                    | ast::Kind::JsxExpression
            )
        {
            return true;
        }
        let _ = store.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }
    false
}

fn insert_statement_after_custom_prologue(
    emit_context: &mut printer::EmitContext,
    mut statements: Vec<ast::Node>,
    statement: ast::Node,
) -> Vec<ast::Node> {
    let mut statement_index = 0;
    while statement_index < statements.len() {
        let current = statements[statement_index];
        let store = emit_context.store_for_node(current);
        if !ast::is_prologue_directive(store, current)
            && emit_context.emit_flags(&current) & printer::EF_CUSTOM_PROLOGUE == 0
        {
            break;
        }
        statement_index += 1;
    }
    statements.insert(statement_index, statement);
    statements
}

struct JsxTransformerRuntime<'ctx, 'source> {
    file: &'ctx ast::SourceFile,
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    compiler_options: &'ctx core::CompilerOptions,
    facts: JsxResolverFacts,
    import_specifier: String,
    utilized_implicit_runtime_imports: Vec<UtilizedImplicitRuntimeImportEntry>,
    in_jsx_child: bool,
    filename_declaration: Option<ast::Node>,
}

impl JsxTransformerRuntime<'_, '_> {
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

    fn subtree_contains_jsx(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        store
            .subtree_facts(node)
            .contains(ast::SubtreeFacts::CONTAINS_JSX)
            || contains_jsx_node(self.source, self.factory(), node)
    }

    fn semantic_jsx_children(&self, children: &[ast::Node]) -> Vec<ast::Node> {
        children
            .iter()
            .copied()
            .filter(|child| {
                let store = self.store_for(*child);
                match store.kind(*child) {
                    ast::Kind::JsxExpression => store.expression(*child).is_some(),
                    ast::Kind::JsxText => !store
                        .contains_only_trivia_white_spaces(*child)
                        .unwrap_or(false),
                    _ => true,
                }
            })
            .collect()
    }

    fn visit(&mut self, node: ast::Node) -> Option<ast::Node> {
        let store = self.store_for(node);
        let kind = store.kind(node);
        let facts = jsx::JsxFacts {
            subtree_contains_jsx: self.subtree_contains_jsx(node)
                || matches!(
                    kind,
                    ast::Kind::JsxElement
                        | ast::Kind::JsxSelfClosingElement
                        | ast::Kind::JsxFragment
                        | ast::Kind::JsxText
                        | ast::Kind::JsxExpression
                ),
            is_declaration_file: ast::is_source_file(store, node)
                && store.source_file_view(node).is_declaration_file(),
            ..Default::default()
        };
        match jsx::jsx_action_for_kind(kind, facts) {
            jsx::JsxAction::Keep => Some(self.preserve_node(node)),
            jsx::JsxAction::SkipSourceFile => Some(self.preserve_node(node)),
            jsx::JsxAction::TransformSourceFile => Some(self.visit_source_file(node)),
            jsx::JsxAction::TransformJsxElement => Some(self.visit_jsx_element(node)),
            jsx::JsxAction::TransformJsxSelfClosingElement => {
                Some(self.visit_jsx_self_closing_element(node))
            }
            jsx::JsxAction::TransformJsxFragment => Some(self.visit_jsx_fragment(node)),
            jsx::JsxAction::TransformJsxText => self.visit_jsx_text(node),
            jsx::JsxAction::TransformJsxExpression => self.visit_jsx_expression(node),
            jsx::JsxAction::VisitChildren => {
                if kind == ast::Kind::ArrowFunction {
                    return Some(self.visit_arrow_function(node));
                }
                Some(self.generated_visit_each_child(&node))
            }
        }
    }

    fn visit_source_file(&mut self, node: ast::Node) -> ast::Node {
        self.in_jsx_child = false;
        self.import_specifier = ast::get_jsx_implicit_import_base(self.compiler_options, self.file);
        self.filename_declaration = None;
        self.utilized_implicit_runtime_imports.clear();
        let source_file_view = self.store_for(node).source_file_view(node);
        let is_external_module = ast::is_external_module(&source_file_view);
        let is_external_or_common_js_module =
            ast::is_external_or_common_js_module(&source_file_view);
        let visited = self.generated_visit_each_child(&node);
        let (source_statements, end_of_file_token) = {
            let source = self.store_for(visited);
            (
                ast::SourceNodeListInput::from_source(
                    source
                        .source_statements(visited)
                        .expect("source file should have statements"),
                ),
                source.source_file_view(visited).end_of_file_token(),
            )
        };
        let source_statement_nodes = source_statements.nodes();
        let mut statements = Vec::with_capacity(source_statement_nodes.len());
        statements.extend(source_statement_nodes);
        let mut statements_updated = false;
        if let Some(filename_declaration) = self.filename_declaration {
            let statement =
                create_filename_declaration_statement(self.factory_mut(), filename_declaration);
            statements_updated = true;
            statements =
                insert_statement_after_custom_prologue(self.emit_context, statements, statement);
        }
        if !self.utilized_implicit_runtime_imports.is_empty() {
            if is_external_module {
                let import_sources = self
                    .utilized_implicit_runtime_imports
                    .iter()
                    .map(|entry| entry.import_source.clone())
                    .collect::<Vec<_>>();
                for import_source in import_sources {
                    let statement = self.create_implicit_runtime_import_declaration(&import_source);
                    statements_updated = true;
                    statements = insert_statement_after_custom_prologue(
                        self.emit_context,
                        statements,
                        statement,
                    );
                }
            } else if is_external_or_common_js_module {
                let import_sources = self
                    .utilized_implicit_runtime_imports
                    .iter()
                    .map(|entry| entry.import_source.clone())
                    .collect::<Vec<_>>();
                for import_source in import_sources {
                    let statement = self.create_implicit_runtime_require_statement(&import_source);
                    statements_updated = true;
                    statements = insert_statement_after_custom_prologue(
                        self.emit_context,
                        statements,
                        statement,
                    );
                }
            }
        }
        if !statements_updated {
            self.import_specifier.clear();
            self.filename_declaration = None;
            self.utilized_implicit_runtime_imports.clear();
            return visited;
        }

        let statement_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            statements,
        );
        let result = if self.is_factory_node(visited) {
            self.factory_mut().update_source_file_in_current_store(
                visited,
                statement_list,
                end_of_file_token,
            )
        } else {
            self.import_state.update_source_file_from_store(
                self.source,
                &mut self.emit_context.factory.node_factory,
                visited,
                statement_list,
                end_of_file_token,
            )
        };
        self.import_specifier.clear();
        self.filename_declaration = None;
        self.utilized_implicit_runtime_imports.clear();
        result
    }

    fn visit_jsx_element(&mut self, node: ast::Node) -> ast::Node {
        let (opening_element, children) = {
            let source = self.store_for(node);
            (
                source.jsx_opening_element(node),
                ast::SourceNodeListInput::from_source(source.jsx_children(node)),
            )
        };
        let loc = self.jsx_location(node);
        if self.should_use_create_element(node) {
            self.visit_jsx_opening_like_element_create_element(opening_element, Some(children), loc)
        } else {
            self.visit_jsx_opening_like_element_jsx(opening_element, Some(children), loc)
        }
    }

    fn visit_jsx_self_closing_element(&mut self, node: ast::Node) -> ast::Node {
        let loc = self.jsx_location(node);
        if self.should_use_create_element(node) {
            self.visit_jsx_opening_like_element_create_element(node, None, loc)
        } else {
            self.visit_jsx_opening_like_element_jsx(node, None, loc)
        }
    }

    fn visit_jsx_fragment(&mut self, node: ast::Node) -> ast::Node {
        let (opening_fragment, children) = {
            let source = self.store_for(node);
            (
                source
                    .opening_fragment(node)
                    .expect("JSX fragment should have an opening fragment"),
                ast::SourceNodeListInput::from_source(source.jsx_children(node)),
            )
        };
        let loc = self.jsx_location(node);
        if self.should_use_create_element(node) {
            self.visit_jsx_opening_fragment_create_element(opening_fragment, Some(children), loc)
        } else {
            self.visit_jsx_opening_fragment_jsx(opening_fragment, Some(children), loc)
        }
    }

    fn should_use_create_element(&self, node: ast::Node) -> bool {
        ast::get_jsx_implicit_import_base(self.compiler_options, self.file).is_empty()
            || self.has_key_after_props_spread(node)
    }

    /**
     * The react jsx/jsxs transform falls back to `createElement` when an explicit `key` argument comes after a spread
     */
    fn has_key_after_props_spread(&self, node: ast::Node) -> bool {
        let mut spread = false;
        let source = self.store_for(node);
        let opener = if source.kind(node) == ast::Kind::JsxElement {
            source.jsx_opening_element(node)
        } else {
            node
        };
        let attrs = self
            .store_for(opener)
            .attributes(opener)
            .and_then(|attributes| self.store_for(attributes).properties(attributes));
        let Some(attrs) = attrs else {
            return false;
        };
        for attr in attrs.iter() {
            let attr_store = self.store_for(attr);
            if attr_store.kind(attr) == ast::Kind::JsxSpreadAttribute {
                if let Some(expression) = attr_store.expression(attr) {
                    let expression_store = self.store_for(expression);
                    let has_spread_assignment =
                        ast::is_object_literal_expression(expression_store, expression)
                            && self
                                .store_for(expression)
                                .properties(expression)
                                .is_some_and(|properties| {
                                    properties.iter().any(|property| {
                                        self.store_for(property).kind(property)
                                            == ast::Kind::SpreadAssignment
                                    })
                                });
                    if !ast::is_object_literal_expression(expression_store, expression)
                        || has_spread_assignment
                    {
                        spread = true;
                    }
                }
            } else if spread
                && attr_store.kind(attr) == ast::Kind::JsxAttribute
                && attr_store.name(attr).is_some_and(|name| {
                    let name_store = self.store_for(name);
                    ast::is_identifier(name_store, name) && name_store.text(name) == "key"
                })
            {
                return true;
            }
        }
        false
    }

    fn visit_jsx_opening_like_element_jsx(
        &mut self,
        element: ast::Node,
        children: Option<ast::SourceNodeListInput>,
        loc: core::TextRange,
    ) -> ast::Node {
        let tag_name = self.get_tag_name(element);
        let children_prop = children.as_ref().and_then(|children| {
            let children = children.iter().collect::<Vec<_>>();
            self.convert_jsx_children_to_children_prop_assignment(&children)
        });
        let mut key_attr = None;
        let mut attrs = self
            .store_for(element)
            .attributes(element)
            .and_then(|attributes| self.store_for(attributes).properties(attributes))
            .map(|attrs| attrs.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for (i, attr) in attrs.iter().copied().enumerate() {
            let attr_store = self.store_for(attr);
            if attr_store.kind(attr) == ast::Kind::JsxAttribute
                && attr_store.name(attr).is_some_and(|name| {
                    let name_store = self.store_for(name);
                    ast::is_identifier(name_store, name) && name_store.text(name) == "key"
                })
            {
                key_attr = Some(attr);
                attrs.remove(i);
                break;
            }
        }
        let object = if !attrs.is_empty() {
            self.transform_jsx_attributes_to_object_props(&attrs, children_prop)
        } else {
            let mut object_children = Vec::new();
            if let Some(children_prop) = children_prop {
                object_children.push(children_prop);
            }
            let prop_list = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                object_children,
            );
            self.factory_mut()
                .new_object_literal_expression(prop_list, false)
        };
        self.visit_jsx_opening_like_element_or_fragment_jsx(
            tag_name, object, key_attr, children, loc,
        )
    }

    fn visit_jsx_opening_fragment_jsx(
        &mut self,
        fragment: ast::Node,
        children: Option<ast::SourceNodeListInput>,
        loc: core::TextRange,
    ) -> ast::Node {
        let children_props = children.as_ref().and_then(|children| {
            let children = children.iter().collect::<Vec<_>>();
            self.convert_jsx_children_to_children_prop_object(&children)
        });
        let object = if let Some(children_props) = children_props {
            children_props
        } else {
            let prop_list = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            self.factory_mut()
                .new_object_literal_expression(prop_list, false)
        };
        let tag_name = self.get_implicit_jsx_fragment_reference(fragment);
        self.visit_jsx_opening_like_element_or_fragment_jsx(tag_name, object, None, children, loc)
    }

    fn convert_jsx_children_to_children_prop_object(
        &mut self,
        children: &[ast::Node],
    ) -> Option<ast::Node> {
        let prop = self.convert_jsx_children_to_children_prop_assignment(children)?;
        let prop_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![prop],
        );
        Some(
            self.factory_mut()
                .new_object_literal_expression(prop_list, false),
        )
    }

    fn convert_jsx_children_to_children_prop_assignment(
        &mut self,
        children: &[ast::Node],
    ) -> Option<ast::Node> {
        let non_whitespace_children = self.semantic_jsx_children(children);
        if non_whitespace_children.len() == 1 && {
            let child = non_whitespace_children[0];
            let child_store = self.store_for(child);
            !ast::is_jsx_expression(child_store, child)
                || child_store.dot_dot_dot_token(child).is_none()
        } {
            let result = self.transform_jsx_child_to_expression(non_whitespace_children[0])?;
            let name = self.factory_mut().new_identifier("children");
            return Some(
                self.factory_mut()
                    .new_property_assignment(None, name, None, None, result),
            );
        }
        // For multiple children in the children property array, don't set StartOnNewLine
        // on child elements — the array literal is single-line.
        let mut results = Vec::with_capacity(non_whitespace_children.len());
        for child in non_whitespace_children {
            if let Some(result) = self.transform_jsx_child_to_expression(child) {
                let flags = self.emit_context.emit_flags(&result);
                self.emit_context
                    .set_emit_flags(&result, flags & !printer::EF_START_ON_NEW_LINE);
                results.push(result);
            }
        }
        if results.is_empty() {
            return None;
        }
        let result_list = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            results,
        );
        let children = self
            .factory_mut()
            .new_array_literal_expression(result_list, false);
        let name = self.factory_mut().new_identifier("children");
        Some(
            self.factory_mut()
                .new_property_assignment(None, name, None, None, children),
        )
    }

    fn transform_jsx_child_to_expression(&mut self, node: ast::Node) -> Option<ast::Node> {
        let previous = self.in_jsx_child;
        self.in_jsx_child = true;
        let result = self.visit_node(Some(node));
        self.in_jsx_child = previous;
        result
    }

    fn get_tag_name(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let tag_name = source
            .tag_name(node)
            .expect("JSX opening-like element should have a tag name");
        let tag_name_store = self.store_for(tag_name);
        if ast::is_identifier(tag_name_store, tag_name)
            && scanner::is_intrinsic_jsx_name(&tag_name_store.text(tag_name))
        {
            let text = tag_name_store.text(tag_name);
            self.factory_mut()
                .new_string_literal(text, ast::TokenFlags::NONE)
        } else if ast::is_jsx_namespaced_name(tag_name_store, tag_name) {
            let namespace = tag_name_store
                .namespace(tag_name)
                .map(|node| self.store_for(node).text(node))
                .unwrap_or_default();
            let name = tag_name_store
                .name(tag_name)
                .map(|node| self.store_for(node).text(node))
                .unwrap_or_default();
            self.factory_mut()
                .new_string_literal(format!("{namespace}:{name}"), ast::TokenFlags::NONE)
        } else {
            self.preserve_node(tag_name)
        }
    }

    fn visit_jsx_opening_like_element_create_element(
        &mut self,
        element: ast::Node,
        children: Option<ast::SourceNodeListInput>,
        loc: core::TextRange,
    ) -> ast::Node {
        let tag_name = self.get_tag_name(element);
        let attrs = self
            .store_for(element)
            .attributes(element)
            .and_then(|attributes| self.store_for(attributes).properties(attributes))
            .map(|attrs| attrs.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let object_properties = if !attrs.is_empty() {
            self.transform_jsx_attributes_to_object_props(&attrs, None)
        } else {
            self.factory_mut()
                .new_keyword_expression(ast::Kind::NullKeyword)
        };

        let callee = if self.import_specifier.is_empty() {
            self.create_jsx_factory_expression(element)
        } else {
            self.get_implicit_import_for_name("createElement")
        };
        let mut new_children = Vec::new();
        if let Some(children) = children.as_ref() {
            for child in children.iter() {
                if let Some(child) = self.transform_jsx_child_to_expression(child) {
                    new_children.push(child);
                }
            }
        }

        // Add StartOnNewLine flag only if there are multiple actual children (after filtering)
        if new_children.len() > 1 {
            for child in &new_children {
                self.emit_context
                    .mark_emit_node(child, printer::EF_START_ON_NEW_LINE);
            }
        }

        let mut args = Vec::with_capacity(new_children.len() + 2);
        args.push(tag_name);
        args.push(object_properties);
        args.extend(new_children);
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            args,
        );
        let result = self.factory_mut().new_call_expression(
            callee,
            None,
            None,
            arguments,
            ast::NodeFlags::NONE,
        );
        self.factory_mut().place_transformed_node(result, loc);
        if self.in_jsx_child {
            self.emit_context
                .mark_emit_node(&result, printer::EF_START_ON_NEW_LINE);
        }
        result
    }

    fn visit_jsx_opening_fragment_create_element(
        &mut self,
        fragment: ast::Node,
        children: Option<ast::SourceNodeListInput>,
        loc: core::TextRange,
    ) -> ast::Node {
        let tag_name = self.create_jsx_fragment_factory_expression(fragment);
        let callee = self.create_jsx_factory_expression(fragment);

        let mut new_children = Vec::new();
        if let Some(children) = children.as_ref() {
            for child in children.iter() {
                if let Some(child) = self.transform_jsx_child_to_expression(child) {
                    new_children.push(child);
                }
            }
        }

        // Add StartOnNewLine flag only if there are multiple actual children (after filtering)
        if new_children.len() > 1 {
            for child in &new_children {
                self.emit_context
                    .mark_emit_node(child, printer::EF_START_ON_NEW_LINE);
            }
        }

        let mut args = Vec::with_capacity(new_children.len() + 2);
        args.push(tag_name);
        args.push(
            self.factory_mut()
                .new_keyword_expression(ast::Kind::NullKeyword),
        );
        args.extend(new_children);
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            args,
        );
        let result = self.factory_mut().new_call_expression(
            callee,
            None,
            None,
            arguments,
            ast::NodeFlags::NONE,
        );
        self.factory_mut().place_transformed_node(result, loc);
        if self.in_jsx_child {
            self.emit_context
                .mark_emit_node(&result, printer::EF_START_ON_NEW_LINE);
        }
        result
    }

    fn transform_jsx_attributes_to_object_props(
        &mut self,
        attrs: &[ast::Node],
        children_prop: Option<ast::Node>,
    ) -> ast::Node {
        let target = self.compiler_options.get_emit_script_target();
        if target >= core::ScriptTarget::ES2018 {
            // target has object spreads, can keep as-is
            let props = self.transform_jsx_attributes_to_props(attrs, children_prop);
            let prop_list = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                props,
            );
            return self
                .factory_mut()
                .new_object_literal_expression(prop_list, false);
        }
        self.transform_jsx_attributes_to_expression(attrs, children_prop)
    }

    fn transform_jsx_attributes_to_expression(
        &mut self,
        attrs: &[ast::Node],
        children_prop: Option<ast::Node>,
    ) -> ast::Node {
        let mut expressions = Vec::new();
        let mut properties = Vec::with_capacity(attrs.len());

        for attr in attrs {
            let attr_store = self.store_for(*attr);
            if attr_store.kind(*attr) == ast::Kind::JsxSpreadAttribute {
                if let Some(expression) = attr_store.expression(*attr) {
                    let expression_store = self.store_for(expression);
                    // as an optimization we try to flatten the first level of spread inline object
                    // as if its props would be passed as JSX attributes
                    if ast::is_object_literal_expression(expression_store, expression)
                        && !self.has_proto(expression)
                    {
                        if let Some(object_properties) = expression_store.properties(expression) {
                            let object_properties = object_properties.iter().collect::<Vec<_>>();
                            for prop in object_properties {
                                let (is_spread_assignment, prop_expression) = {
                                    let prop_store = self.store_for(prop);
                                    (
                                        prop_store.kind(prop) == ast::Kind::SpreadAssignment,
                                        prop_store.expression(prop),
                                    )
                                };
                                if is_spread_assignment {
                                    expressions = self.combine_properties_into_new_expression(
                                        expressions,
                                        &mut properties,
                                    );
                                    if let Some(prop_expression) = prop_expression
                                        && let Some(visited) =
                                            self.visit_node(Some(prop_expression))
                                    {
                                        expressions.push(visited);
                                    }
                                    continue;
                                }
                                if let Some(visited) = self.visit_node(Some(prop)) {
                                    properties.push(visited);
                                }
                            }
                        }
                        continue;
                    }
                    expressions =
                        self.combine_properties_into_new_expression(expressions, &mut properties);
                    if let Some(visited) = self.visit_node(Some(expression)) {
                        expressions.push(visited);
                    }
                }
                continue;
            }
            properties.push(self.transform_jsx_attribute_to_object_literal_element(*attr));
        }

        if let Some(children_prop) = children_prop {
            properties.push(children_prop);
        }

        expressions = self.combine_properties_into_new_expression(expressions, &mut properties);

        if expressions.first().is_some_and(|expression| {
            !ast::is_object_literal_expression(self.store_for(*expression), *expression)
        }) {
            // We must always emit at least one object literal before a spread attribute
            // as the JSX always factory expects a fresh object, so we need to make a copy here
            // we also avoid mutating an external reference by doing this (first expression is used as assign's target)
            let properties = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            let empty = self
                .factory_mut()
                .new_object_literal_expression(properties, false);
            expressions.insert(0, empty);
        }

        if expressions.len() == 1 {
            return expressions[0];
        }
        self.emit_context
            .factory
            .new_assign_helper(&expressions, self.compiler_options.get_emit_script_target())
    }

    fn combine_properties_into_new_expression(
        &mut self,
        mut expressions: Vec<ast::Node>,
        props: &mut Vec<ast::Node>,
    ) -> Vec<ast::Node> {
        if props.is_empty() {
            return expressions;
        }
        let properties = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            std::mem::take(props),
        );
        let new_obj = self
            .factory_mut()
            .new_object_literal_expression(properties, false);
        expressions.push(new_obj);
        expressions
    }

    fn transform_jsx_attributes_to_props(
        &mut self,
        attrs: &[ast::Node],
        children_prop: Option<ast::Node>,
    ) -> Vec<ast::Node> {
        let mut props = Vec::with_capacity(attrs.len());
        for attr in attrs {
            if self.store_for(*attr).kind(*attr) == ast::Kind::JsxSpreadAttribute {
                props.extend(self.transform_jsx_spread_attributes_to_props(*attr));
            } else {
                props.push(self.transform_jsx_attribute_to_object_literal_element(*attr));
            }
        }
        if let Some(children_prop) = children_prop {
            props.push(children_prop);
        }
        props
    }

    fn has_proto(&self, obj: ast::Node) -> bool {
        let source = self.store_for(obj);
        let Some(properties) = source.properties(obj) else {
            return false;
        };
        properties.iter().any(|p| {
            let prop_store = self.store_for(p);
            ast::is_property_assignment(prop_store, p)
                && prop_store.name(p).is_some_and(|name| {
                    let name_store = self.store_for(name);
                    (ast::is_string_literal(name_store, name)
                        || ast::is_identifier(name_store, name))
                        && name_store.text(name) == "__proto__"
                })
        })
    }

    fn transform_jsx_spread_attributes_to_props(&mut self, node: ast::Node) -> Vec<ast::Node> {
        let source = self.store_for(node);
        if let Some(expression) = source.expression(node) {
            let expression_store = self.store_for(expression);
            if ast::is_object_literal_expression(expression_store, expression)
                && !self.has_proto(expression)
                && let Some(properties) = expression_store.properties(expression)
            {
                let properties = properties.iter().collect::<Vec<_>>();
                return properties
                    .iter()
                    .filter_map(|prop| self.visit_node(Some(*prop)))
                    .collect();
            }
            if let Some(expression) = self.visit_node(Some(expression)) {
                return vec![self.factory_mut().new_spread_assignment(expression)];
            }
        }
        Vec::new()
    }

    fn transform_jsx_attribute_to_object_literal_element(&mut self, node: ast::Node) -> ast::Node {
        let name = self.get_attribute_name(node);
        let expression =
            self.transform_jsx_attribute_initializer(self.store_for(node).initializer(node));
        self.factory_mut()
            .new_property_assignment(None, name, None, None, expression)
    }

    /**
     * Emit an attribute name, which is quoted if it needs to be quoted. Because
     * these emit into an object literal property name, we don't need to be worried
     * about keywords, just non-identifier characters
     */
    fn get_attribute_name(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let name = source.name(node).expect("JSX attribute should have a name");
        let name_store = self.store_for(name);
        if ast::is_identifier(name_store, name) {
            let text = name_store.text(name);
            if scanner::is_identifier_text(&text, core::LanguageVariant::Standard) {
                return self.preserve_node(name);
            }
            return self
                .factory_mut()
                .new_string_literal(text, ast::TokenFlags::NONE);
        }
        let namespace = name_store
            .namespace(name)
            .expect("JSX namespaced name should have a namespace");
        let name_part = name_store
            .name(name)
            .expect("JSX namespaced name should have a name");
        let text = format!(
            "{}:{}",
            self.store_for(namespace).text(namespace),
            self.store_for(name_part).text(name_part)
        );
        self.factory_mut()
            .new_string_literal(text, ast::TokenFlags::NONE)
    }

    fn transform_jsx_attribute_initializer(&mut self, node: Option<ast::Node>) -> ast::Node {
        let Some(node) = node else {
            return self.emit_context.factory.new_true_expression();
        };
        let source = self.store_for(node);
        let kind = source.kind(node);
        if kind == ast::Kind::StringLiteral {
            // Always recreate the literal to escape any escape sequences or newlines which may be in the original jsx string and which
            // Need to be escaped to be handled correctly in a normal string
            let text = jsx::decode_entities(&source.text(node));
            let token_flags = source.token_flags(node).unwrap_or(ast::TokenFlags::NONE);
            let loc = source.loc(node);
            let result = self.factory_mut().new_string_literal(text, token_flags);
            self.factory_mut().place_transformed_node(result, loc);
            return result;
        }
        if kind == ast::Kind::JsxExpression {
            if let Some(expression) = source.expression(node) {
                return self
                    .visit_node(Some(expression))
                    .unwrap_or_else(|| self.emit_context.factory.new_true_expression());
            }
            return self.emit_context.factory.new_true_expression();
        }
        if ast::is_jsx_element(source, node)
            || ast::is_jsx_self_closing_element(source, node)
            || ast::is_jsx_fragment(source, node)
        {
            let previous = self.in_jsx_child;
            self.in_jsx_child = false;
            let result = self
                .visit_node(Some(node))
                .unwrap_or_else(|| self.emit_context.factory.new_true_expression());
            self.in_jsx_child = previous;
            return result;
        }
        panic!("Unhandled node kind found in jsx initializer: {:?}", kind);
    }

    fn create_jsx_factory_expression(&mut self, parent: ast::Node) -> ast::Node {
        self.create_jsx_pseudo_factory_expression(
            parent,
            self.facts.factory_entity.clone(),
            "createElement",
        )
    }

    fn create_jsx_fragment_factory_expression(&mut self, parent: ast::Node) -> ast::Node {
        self.create_jsx_pseudo_factory_expression(
            parent,
            self.facts.fragment_factory_entity.clone(),
            "Fragment",
        )
    }

    fn create_jsx_pseudo_factory_expression(
        &mut self,
        parent: ast::Node,
        entity: Option<String>,
        target: &str,
    ) -> ast::Node {
        if let Some(entity) = entity {
            return self.create_jsx_factory_expression_from_text(
                &entity,
                parent,
                target == "createElement",
            );
        }
        let react =
            self.create_react_namespace(&self.compiler_options.react_namespace.clone(), parent);
        let target = self.factory_mut().new_identifier(target);
        self.factory_mut()
            .new_property_access_expression(react, None, target, ast::NodeFlags::NONE)
    }

    fn create_jsx_factory_expression_from_text(
        &mut self,
        text: &str,
        parent: ast::Node,
        indirect_call: bool,
    ) -> ast::Node {
        let mut parts = text.split('.');
        let first = parts.next().unwrap_or("React");
        if parts.clone().next().is_none()
            && let Some(reference) = self.imported_factory_reference(first, indirect_call)
        {
            return reference;
        }
        let mut expression = self.create_react_namespace(first, parent);
        for part in parts {
            let name = self.factory_mut().new_identifier(part);
            expression = self.factory_mut().new_property_access_expression(
                expression,
                None,
                name,
                ast::NodeFlags::NONE,
            );
        }
        expression
    }

    fn imported_factory_reference(&mut self, name: &str, indirect_call: bool) -> Option<ast::Node> {
        let (import_declaration, property_name) = self.import_reference_for_name(name)?;
        let target = self
            .emit_context
            .new_generated_name_for_node(import_declaration);
        let property_name = self.factory_mut().new_identifier(property_name);
        self.emit_context.mark_emit_node(
            &property_name,
            printer::EF_NO_SOURCE_MAP | printer::EF_NO_COMMENTS,
        );
        let reference = self.factory_mut().new_property_access_expression(
            target,
            None,
            property_name,
            ast::NodeFlags::NONE,
        );
        if !indirect_call {
            return Some(reference);
        }
        let zero = self
            .factory_mut()
            .new_numeric_literal("0", ast::TokenFlags::NONE);
        Some(
            self.emit_context
                .factory
                .new_comma_expression(zero, reference),
        )
    }

    fn import_reference_for_name(&self, name: &str) -> Option<(ast::Node, String)> {
        let statements = self
            .source
            .parser_access()
            .source_file_statement_list(self.file.root());
        for statement in statements.iter() {
            if !ast::is_import_declaration(self.source, statement) {
                continue;
            }
            let Some(import_clause) = self.source.import_clause(statement) else {
                continue;
            };
            if let Some(default_name) = self.source.name(import_clause)
                && self.source.text(default_name) == name
            {
                return Some((statement, "default".to_string()));
            }
            let Some(named_bindings) = self.source.named_bindings(import_clause) else {
                continue;
            };
            if self.source.kind(named_bindings) != ast::Kind::NamedImports {
                continue;
            }
            let Some(elements) = self.source.source_elements(named_bindings) else {
                continue;
            };
            for specifier in elements.iter() {
                let Some(local_name) = self.source.name(specifier) else {
                    continue;
                };
                if self.source.text(local_name) == name {
                    let property_name = self
                        .source
                        .property_name_or_name(specifier)
                        .map(|property_name| self.source.text(property_name))
                        .unwrap_or_else(|| name.to_string());
                    return Some((statement, property_name));
                }
            }
        }
        None
    }

    fn create_react_namespace(&mut self, react_namespace: &str, parent: ast::Node) -> ast::Node {
        let react_namespace = if react_namespace.is_empty() {
            "React"
        } else {
            react_namespace
        };
        // To ensure the emit resolver can properly resolve the namespace, we need to
        // treat this identifier as if it were a source tree node by clearing the `Synthesized`
        // flag and setting a parent node. TODO: Is this still true? The emit resolver is supposed to be
        // hardened against this, so long as the node retains original node pointers back to a parsed node
        let react = self.factory_mut().new_identifier(react_namespace);
        self.factory_mut().clear_emit_synthetic_node(react);

        // Set the parent that is in parse tree
        // this makes sure that parent chain is intact for checker to traverse complete scope tree
        self.emit_context.unset_original(&react);
        let parent = self.emit_context.parse_node(&parent);
        self.factory_mut().link_emit_synthetic_parent(react, parent);

        // If the identifier refers to an exported member of a namespace, substitute with
        // a qualified namespace property access (e.g., `React` -> `M.React`).
        // See also: RuntimeSyntaxTransformer.visitExpressionIdentifier in runtimesyntax.go
        let referenced_export_container = parent.and_then(|parent| {
            let store = self.store_for(parent);
            self.facts
                .referenced_export_containers
                .get(&(store.loc(parent), react_namespace.to_string()))
                .copied()
        });
        if let Some(container) = referenced_export_container
            && ast::is_module_declaration(self.store_for(container), container)
        {
            let container_name = self.emit_context.new_generated_name_for_node(container);
            return self.factory_mut().new_property_access_expression(
                container_name,
                None,
                react,
                ast::NodeFlags::NONE,
            );
        }

        react
    }

    fn get_implicit_jsx_fragment_reference(&mut self, _parent: ast::Node) -> ast::Node {
        self.get_implicit_import_for_name("Fragment")
    }

    fn visit_jsx_opening_like_element_or_fragment_jsx(
        &mut self,
        tag_name: ast::Node,
        object: ast::Node,
        key_attr: Option<ast::Node>,
        children: Option<ast::SourceNodeListInput>,
        loc: core::TextRange,
    ) -> ast::Node {
        let non_whitespace_children = children
            .as_ref()
            .map(|children| {
                let children = children.iter().collect::<Vec<_>>();
                self.semantic_jsx_children(&children)
            })
            .unwrap_or_default();
        let is_static_children = non_whitespace_children.len() > 1
            || (non_whitespace_children.len() == 1 && {
                let child = non_whitespace_children[0];
                let child_store = self.store_for(child);
                ast::is_jsx_expression(child_store, child)
                    && child_store.dot_dot_dot_token(child).is_some()
            });
        let jsx = self.compiler_options.jsx;
        let callee = self.get_implicit_import_for_name(jsx::jsx_factory_callee_primitive(
            jsx,
            is_static_children,
        ));
        let mut args = vec![tag_name, object];
        // function jsx(type, config, maybeKey) {}
        // "maybeKey" is optional. It is acceptable to use "_jsx" without a third argument
        if let Some(key_attr) = key_attr {
            args.push(self.transform_jsx_attribute_initializer(
                self.store_for(key_attr).initializer(key_attr),
            ));
        }
        if jsx == core::JsxEmit::ReactJSXDev {
            // "maybeKey" has to be replaced with "void 0" to not break the jsxDEV signature
            if key_attr.is_none() {
                args.push(self.emit_context.factory.new_void_zero_expression());
            }
            args.push(if is_static_children {
                self.emit_context.factory.new_true_expression()
            } else {
                self.emit_context.factory.new_false_expression()
            });
            let (line, col) =
                scanner::get_ecma_line_and_utf16_character_of_position(self.file, loc.pos());
            let file_name = self.get_current_file_name_expression();
            let file_name_property = self.create_property_assignment("fileName", file_name);
            let line_number = self
                .factory_mut()
                .new_numeric_literal((line + 1).to_string(), ast::TokenFlags::NONE);
            let line_number_property = self.create_property_assignment("lineNumber", line_number);
            let column_number = self
                .factory_mut()
                .new_numeric_literal((col + 1).to_string(), ast::TokenFlags::NONE);
            let column_number_property =
                self.create_property_assignment("columnNumber", column_number);
            let properties = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![
                    file_name_property,
                    line_number_property,
                    column_number_property,
                ],
            );
            args.push(
                self.factory_mut()
                    .new_object_literal_expression(properties, false),
            );
            args.push(self.emit_context.factory.new_this_expression());
        }
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            args,
        );
        self.factory_mut()
            .new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE)
    }

    fn jsx_location(&self, node: ast::Node) -> core::TextRange {
        let loc = self.store_for(node).loc(node);
        let pos = scanner::skip_trivia(self.file.data().text(), loc.pos().max(0) as usize) as i32;
        core::new_text_range(pos, loc.end())
    }

    fn get_implicit_import_for_name(&mut self, name: &str) -> ast::Node {
        let import_source = if name == "createElement" {
            self.import_specifier.clone()
        } else {
            ast::get_jsx_runtime_import(&self.import_specifier, self.compiler_options)
        };
        let entry_index = self
            .utilized_implicit_runtime_imports
            .iter()
            .position(|entry| entry.import_source == import_source);
        if let Some(entry_index) = entry_index {
            if let Some(existing) = self.utilized_implicit_runtime_imports[entry_index]
                .imports
                .iter()
                .find(|import| {
                    let store = self.factory().store();
                    store
                        .property_name(import.import_specifier)
                        .is_some_and(|property_name| store.text(property_name) == name)
                })
            {
                return existing.name;
            }
        }

        let generated_name = self.emit_context.factory.new_unique_name_ex(
            &format!("_{name}"),
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::OPTIMISTIC
                    | GeneratedIdentifierFlags::FILE_LEVEL
                    | GeneratedIdentifierFlags::ALLOW_NAME_SUBSTITUTION,
                ..Default::default()
            },
        );
        let property_name = self.factory_mut().new_identifier(name);
        let specifier = self.factory_mut().new_import_specifier(
            false,
            Some(property_name),
            Some(generated_name),
        );
        let import = UtilizedImplicitRuntimeImport {
            import_specifier: specifier,
            name: generated_name,
        };
        if let Some(entry_index) = entry_index {
            self.utilized_implicit_runtime_imports[entry_index]
                .imports
                .push(import);
        } else {
            self.utilized_implicit_runtime_imports
                .push(UtilizedImplicitRuntimeImportEntry {
                    import_source,
                    imports: vec![import],
                });
        }
        generated_name
    }

    fn sorted_implicit_runtime_import_specifiers(&self, import_source: &str) -> Vec<ast::Node> {
        let mut specifiers = self
            .utilized_implicit_runtime_imports
            .iter()
            .find(|entry| entry.import_source == import_source)
            .map(|entry| entry.imports.clone())
            .unwrap_or_default();
        specifiers.sort_by(|a, b| {
            let store = self.factory().store();
            let a_property = store
                .property_name(a.import_specifier)
                .map(|name| store.text(name))
                .unwrap_or_default();
            let b_property = store
                .property_name(b.import_specifier)
                .map(|name| store.text(name))
                .unwrap_or_default();
            a_property.cmp(&b_property).then_with(|| {
                let a_name = store
                    .name(a.import_specifier)
                    .map(|name| store.text(name))
                    .unwrap_or_default();
                let b_name = store
                    .name(b.import_specifier)
                    .map(|name| store.text(name))
                    .unwrap_or_default();
                a_name.cmp(&b_name)
            })
        });
        specifiers
            .into_iter()
            .map(|specifier| specifier.import_specifier)
            .collect()
    }

    fn create_implicit_runtime_import_declaration(&mut self, import_source: &str) -> ast::Node {
        let specifiers = self.sorted_implicit_runtime_import_specifiers(import_source);
        let elements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            specifiers,
        );
        let named_imports = self.factory_mut().new_named_imports(elements);
        let import_clause = self.factory_mut().new_import_clause(
            None::<ast::Kind>,
            None::<ast::Node>,
            Some(named_imports),
        );
        let module_specifier = self
            .factory_mut()
            .new_string_literal(import_source, ast::TokenFlags::NONE);
        self.factory_mut().new_import_declaration(
            None::<ast::ModifierList>,
            Some(import_clause),
            Some(module_specifier),
            None::<ast::Node>,
        )
    }

    fn create_implicit_runtime_require_statement(&mut self, import_source: &str) -> ast::Node {
        let specifiers = self.sorted_implicit_runtime_import_specifiers(import_source);
        let mut binding_elements = Vec::with_capacity(specifiers.len());
        for specifier in specifiers {
            let property_name = self
                .factory()
                .store()
                .property_name(specifier)
                .expect("JSX runtime import specifier should have a property name");
            let name = self
                .factory()
                .store()
                .name(specifier)
                .expect("JSX runtime import specifier should have a name");
            let property_name = self.preserve_node(property_name);
            let name = self.preserve_node(name);
            binding_elements.push(self.factory_mut().new_binding_element(
                None::<ast::Node>,
                Some(property_name),
                Some(name),
                None::<ast::Node>,
            ));
        }
        let elements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            binding_elements,
        );
        let name = self
            .factory_mut()
            .new_binding_pattern(ast::Kind::ObjectBindingPattern, elements);
        let require = self.factory_mut().new_identifier("require");
        let runtime_import = self
            .factory_mut()
            .new_string_literal(import_source, ast::TokenFlags::NONE);
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![runtime_import],
        );
        let initializer = self.factory_mut().new_call_expression(
            require,
            None,
            None,
            arguments,
            ast::NodeFlags::NONE,
        );
        let declaration =
            self.factory_mut()
                .new_variable_declaration(name, None, None, initializer);
        create_variable_statement(self.factory_mut(), declaration)
    }

    fn get_current_file_name_expression(&mut self) -> ast::Node {
        if let Some(declaration) = self.filename_declaration {
            return self
                .factory()
                .store()
                .name(declaration)
                .expect("JSX file name declaration should have a name");
        }
        let name = self.emit_context.factory.new_unique_name_ex(
            "_jsxFileName",
            AutoGenerateOptions {
                flags: GeneratedIdentifierFlags::OPTIMISTIC | GeneratedIdentifierFlags::FILE_LEVEL,
                ..Default::default()
            },
        );
        let file_name_text = self.file.file_name().to_string();
        let file_name = self
            .factory_mut()
            .new_string_literal(&file_name_text, ast::TokenFlags::NONE);
        let declaration = self
            .factory_mut()
            .new_variable_declaration(name, None, None, file_name);
        self.filename_declaration = Some(declaration);
        name
    }

    fn create_property_assignment(&mut self, name: &str, initializer: ast::Node) -> ast::Node {
        let name = self.factory_mut().new_identifier(name);
        self.factory_mut()
            .new_property_assignment(None, name, None, None, initializer)
    }

    fn visit_jsx_text(&mut self, node: ast::Node) -> Option<ast::Node> {
        let text = self.store_for(node).text(node);
        jsx::jsx_text_to_string_literal(&text).map(|text| {
            self.factory_mut()
                .new_string_literal(text, ast::TokenFlags::NONE)
        })
    }

    fn visit_jsx_expression(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (expression, dot_dot_dot_token) = {
            let source = self.store_for(node);
            (source.expression(node), source.dot_dot_dot_token(node))
        };
        let expression = expression.and_then(|expression| self.visit_node(Some(expression)))?;
        if dot_dot_dot_token.is_some() {
            Some(self.factory_mut().new_spread_element(expression))
        } else {
            Some(expression)
        }
    }

    fn visit_arrow_function(&mut self, node: ast::Node) -> ast::Node {
        let (modifiers, parameters, equals_greater_than_token, body) = {
            let source = self.store_for(node);
            (
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.equals_greater_than_token(node),
                source.body(node),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers);
        let parameters = self
            .visit_parameters_input(parameters)
            .expect("arrow function parameters are required");
        let equals_greater_than_token =
            equals_greater_than_token.map(|token| self.preserve_node(token));
        let body = self.visit_node(body);
        if self.is_factory_node(node) {
            self.factory_mut().update_arrow_function(
                node,
                modifiers,
                None,
                parameters,
                None,
                None,
                equals_greater_than_token,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_arrow_function_from_store(
                source,
                node,
                modifiers,
                None,
                parameters,
                None,
                None,
                equals_greater_than_token,
                body,
            )
        }
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for JsxTransformerRuntime<'_, 'source> {
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
        let imported = self.import_state.preserve_node(
            source,
            &mut self.emit_context.factory.node_factory,
            node,
        );
        self.emit_context.set_original(&imported, &node);
        imported
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
        if self.is_factory_node(node) {
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
        let mut visited = self.visit(node)?;
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

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node?;
        self.emit_context.begin_visit_iteration_body();
        let updated = self.visit_embedded_statement(node);
        self.emit_context.finish_visit_iteration_body(updated)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for JsxTransformerRuntime<'_, 'source> {}

fn create_filename_declaration_statement(
    factory: &mut ast::NodeFactory,
    declaration: ast::Node,
) -> ast::Node {
    create_variable_statement(factory, declaration)
}

fn create_variable_statement(factory: &mut ast::NodeFactory, declaration: ast::Node) -> ast::Node {
    let declarations = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![declaration],
    );
    let declaration_list =
        factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
    factory.new_variable_statement(None, declaration_list)
}

fn transform_jsx_self_closing_element(
    source: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: &ast::Node,
) -> ast::Node {
    let tag_name = source
        .tag_name(*node)
        .expect("jsx self-closing element should have a tag name");
    let tag = if ast::is_identifier(source, tag_name) {
        factory.new_string_literal(source.text(tag_name), ast::TokenFlags::NONE)
    } else {
        tag_name
    };
    let zero = factory.new_numeric_literal("0", ast::TokenFlags::NONE);
    let comma = factory.new_token(ast::Kind::CommaToken);
    let runtime = factory.new_identifier("jsx_runtime_1");
    let callee_name = factory.new_identifier(jsx::jsx_factory_callee_primitive(
        core::JsxEmit::ReactJSX,
        false,
    ));
    let runtime_callee =
        factory.new_property_access_expression(runtime, None, callee_name, ast::NodeFlags::NONE);
    let binary = factory.new_binary_expression(None, zero, None, comma, runtime_callee);
    let callee = factory.new_parenthesized_expression(binary);
    let prop_list = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        Vec::<ast::Node>::new(),
    );
    let props = factory.new_object_literal_expression(prop_list, false);
    let arguments = factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        vec![tag, props],
    );
    factory.new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_file(is_declaration_file: bool) -> ast::SourceFile {
        let mut factory = ast::NodeFactory::default();
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::new(),
        );
        let root = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: "/jsx.tsx".to_string(),
                path: "/jsx.tsx".to_string(),
                ..Default::default()
            },
            String::new(),
            statements,
            None,
        );
        factory.finish_parsed_source_file(
            root,
            ast::ParsedSourceFileMetadata {
                is_declaration_file,
                ..Default::default()
            },
        )
    }

    fn transform_jsx(file: &ast::SourceFile, options: &core::CompilerOptions) -> ast::SourceFile {
        let mut transformer = Transformer::default();
        transformer.new_source_file_transformer(
            SourceFileTransformer::Jsx {
                compiler_options: options.clone(),
                facts: JsxResolverFacts::default(),
            },
            Some(printer::new_emit_context()),
        );
        transformer.transform_source_file(file)
    }

    #[test]
    fn jsx_source_file_gate_matches_go_action_table_for_non_jsx_and_declarations() {
        let options = core::CompilerOptions::default();
        let plain = source_file(false);
        let declaration = source_file(true);

        assert_eq!(
            jsx::jsx_action_for_kind(
                ast::Kind::SourceFile,
                jsx::JsxFacts {
                    subtree_contains_jsx: false,
                    ..Default::default()
                }
            ),
            jsx::JsxAction::Keep
        );
        assert_eq!(
            transform_jsx(&plain, &options).data().file_name(),
            plain.data().file_name()
        );
        assert_eq!(
            transform_jsx(&declaration, &options).data().file_name(),
            declaration.data().file_name()
        );
    }
}
