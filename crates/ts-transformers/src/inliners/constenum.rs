use std::collections::HashMap;
use std::ops::ControlFlow;

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_evaluator as evaluator;
use ts_printer as printer;
use ts_scanner as scanner;

#[derive(Clone, Debug, PartialEq)]
pub enum ConstEnumValue {
    Number(f64),
    String(String),
    BigInt {
        base10_value: String,
        negative: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstEnumReplacement {
    NumericLiteral(String),
    StringLiteral(String),
    BigIntLiteral(String),
    Identifier(&'static str),
    NegativeInfinity,
    NegativeNumericLiteral(String),
    NegativeBigIntLiteral(String),
}

pub fn const_enum_replacement(value: ConstEnumValue) -> ConstEnumReplacement {
    match value {
        ConstEnumValue::Number(value) if value.is_infinite() && value.is_sign_positive() => {
            ConstEnumReplacement::Identifier("Infinity")
        }
        ConstEnumValue::Number(value) if value.is_infinite() => {
            ConstEnumReplacement::NegativeInfinity
        }
        ConstEnumValue::Number(value) if value.is_nan() => ConstEnumReplacement::Identifier("NaN"),
        ConstEnumValue::Number(value) if value.is_sign_positive() || value == 0.0 => {
            ConstEnumReplacement::NumericLiteral(format_number(value))
        }
        ConstEnumValue::Number(value) => {
            ConstEnumReplacement::NegativeNumericLiteral(format_number(value.abs()))
        }
        ConstEnumValue::String(value) => ConstEnumReplacement::StringLiteral(value),
        ConstEnumValue::BigInt {
            base10_value,
            negative: false,
        } if base10_value.is_empty() => ConstEnumReplacement::BigIntLiteral("0".to_owned()),
        ConstEnumValue::BigInt {
            base10_value,
            negative: false,
        } => ConstEnumReplacement::BigIntLiteral(base10_value),
        ConstEnumValue::BigInt {
            base10_value,
            negative: true,
        } => ConstEnumReplacement::NegativeBigIntLiteral(base10_value),
    }
}

pub fn should_inline_const_enum_access(kind: ast::Kind, has_constant_value: bool) -> bool {
    matches!(
        kind,
        ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression
    ) && has_constant_value
}

pub fn should_add_original_access_comment(
    remove_comments: bool,
    original_is_synthesized: bool,
) -> bool {
    !remove_comments && !original_is_synthesized
}

pub fn safe_multi_line_comment(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 2);
    result.push(' ');
    result.push_str(&text.replace("*/", "*_/"));
    result.push(' ');
    result
}

#[derive(Clone, Default)]
pub struct ConstEnumInliningFacts {
    values: HashMap<core::TextRange, evaluator::Value>,
}

impl ConstEnumInliningFacts {
    fn get(&self, store: &ast::AstStore, node: ast::Node) -> Option<&evaluator::Value> {
        self.values.get(&store.loc(node))
    }

    fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

pub fn collect_const_enum_inlining_facts(
    source_file: &ast::SourceFile,
    resolver: &mut dyn printer::EmitResolver,
) -> ConstEnumInliningFacts {
    let mut facts = ConstEnumInliningFacts::default();
    let store = source_file.store();
    let mut stack = vec![source_file.root()];
    while let Some(node) = stack.pop() {
        match store.kind(node) {
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                if !ast::is_part_of_type_node(store, node)
                    && let Some(value) = resolver.get_constant_value(node)
                    && value.is_some()
                {
                    facts.values.insert(store.loc(node), value);
                }
            }
            _ => {}
        }
        let _ = store.for_each_present_child(node, |child| {
            stack.push(child);
            ControlFlow::Continue(())
        });
    }
    facts
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    facts: &ConstEnumInliningFacts,
) -> ast::Node {
    if facts.is_empty() {
        return root;
    }
    let mut runtime = ConstEnumInliningRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        compiler_options,
        facts,
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

struct ConstEnumInliningRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    compiler_options: &'ctx core::CompilerOptions,
    facts: &'ctx ConstEnumInliningFacts,
}

impl ConstEnumInliningRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        self.emit_context.store_for_node(node)
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        match source.kind(*node) {
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                if let Some(replacement) = self.const_enum_replacement_for_access(*node) {
                    return Some(replacement);
                }
            }
            _ => {}
        }
        Some(self.generated_visit_each_child(node))
    }

    fn const_enum_replacement_for_access(&mut self, node: ast::Node) -> Option<ast::Node> {
        let parse_node = self.emit_context.parse_node(&node)?;
        let parse_store = self.store_for(parse_node);
        let value = self.facts.get(parse_store, parse_node)?.clone();
        let replacement = self.create_const_enum_replacement(value)?;
        if !self.compiler_options.remove_comments.is_true() {
            let original = self.emit_context.most_original(&node);
            let original_store = self.store_for(original);
            if !ast::node_is_synthesized(original_store, original) {
                let original_source_file = self.emit_context.source_file_for_node(original)?;
                let original_text = scanner::get_text_of_node(&original_source_file, &original);
                self.emit_context.add_synthetic_trailing_comment(
                    &replacement,
                    ast::Kind::MultiLineCommentTrivia,
                    safe_multi_line_comment(&original_text),
                    false,
                );
            }
        }
        Some(replacement)
    }

    fn create_const_enum_replacement(&mut self, value: evaluator::Value) -> Option<ast::Node> {
        let replacement = match value {
            evaluator::Value::Number(value) => {
                const_enum_replacement(ConstEnumValue::Number(value.0))
            }
            evaluator::Value::String(value) => {
                const_enum_replacement(ConstEnumValue::String(value))
            }
            evaluator::Value::PseudoBigInt(value) => {
                const_enum_replacement(ConstEnumValue::BigInt {
                    base10_value: value.base10_value,
                    negative: value.negative,
                })
            }
            evaluator::Value::None | evaluator::Value::Bool(_) => return None,
        };
        Some(match replacement {
            ConstEnumReplacement::NumericLiteral(text) => self
                .factory_mut()
                .new_numeric_literal(text, ast::TokenFlags::NONE),
            ConstEnumReplacement::StringLiteral(text) => self
                .factory_mut()
                .new_string_literal(text, ast::TokenFlags::NONE),
            ConstEnumReplacement::BigIntLiteral(text) => self
                .factory_mut()
                .new_big_int_literal(text, ast::TokenFlags::NONE),
            ConstEnumReplacement::Identifier(text) => self.factory_mut().new_identifier(text),
            ConstEnumReplacement::NegativeInfinity => {
                let infinity = self.factory_mut().new_identifier("Infinity");
                self.factory_mut()
                    .new_prefix_unary_expression(ast::Kind::MinusToken, infinity)
            }
            ConstEnumReplacement::NegativeNumericLiteral(text) => {
                let literal = self
                    .factory_mut()
                    .new_numeric_literal(text, ast::TokenFlags::NONE);
                self.factory_mut()
                    .new_prefix_unary_expression(ast::Kind::MinusToken, literal)
            }
            ConstEnumReplacement::NegativeBigIntLiteral(text) => {
                let literal = self
                    .factory_mut()
                    .new_big_int_literal(text, ast::TokenFlags::NONE);
                self.factory_mut()
                    .new_prefix_unary_expression(ast::Kind::MinusToken, literal)
            }
        })
    }

    fn append_visited_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
        changed: &mut bool,
    ) {
        match visited {
            Some(visited) if self.visited_node_preserves_original(original, visited) => {
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

    fn visited_node_preserves_original(&self, original: ast::Node, visited: ast::Node) -> bool {
        if original.store_id() == self.factory().store().store_id() {
            original == visited
        } else {
            self.preserved_source_node_matches(Some(original), Some(visited))
        }
    }

    fn preserve_source_node_list_input(
        &mut self,
        nodes: &ast::SourceNodeListInput,
    ) -> ast::NodeList {
        if nodes.store_id() == self.factory().store().store_id() {
            return nodes.as_node_list();
        }
        self.import_state.preserve_source_node_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            nodes,
        )
    }

    fn preserve_source_modifier_list_input(
        &mut self,
        modifiers: &ast::SourceModifierListInput,
    ) -> ast::ModifierList {
        if modifiers.store_id() == self.factory().store().store_id() {
            return modifiers.as_modifier_list();
        }
        self.import_state.preserve_source_modifier_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            modifiers,
        )
    }

    fn preserve_source_raw_node_slice_input(
        &mut self,
        nodes: &ast::SourceRawNodeSliceInput,
    ) -> ast::RawNodeSlice {
        if nodes.store_id() == self.factory().store().store_id() {
            return nodes.as_raw_node_slice();
        }
        self.import_state.preserve_source_raw_node_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            nodes,
        )
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

impl<'source> ast::AstVisitEachChildRuntime<'source> for ConstEnumInliningRuntime<'_, 'source> {
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
            return self
                .emit_context
                .factory
                .node_factory
                .update_source_file_in_current_store(
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
            Some(self.preserve_source_node_list_input(&nodes))
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for ConstEnumInliningRuntime<'_, 'source> {}
