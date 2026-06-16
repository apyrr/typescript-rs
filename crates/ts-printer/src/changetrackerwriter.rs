use std::{cell::RefCell, collections::HashMap, rc::Rc};

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::{DEFAULT_INDENT_SIZE, EmitTextWriter, PrintHandlers, TextWriter, UTF16Offset};

#[derive(Clone)]
pub struct ChangeTrackerWriter {
    inner: Rc<RefCell<ChangeTrackerWriterState>>,
}

struct ChangeTrackerWriterState {
    text_writer: TextWriter,
    last_non_trivia_position: i32,
    pos: HashMap<TriviaPositionKey, i32>,
    end: HashMap<TriviaPositionKey, i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TriviaPositionKey {
    Node(ast::Node),
    NodeList(ast::NodeListPositionKey),
}

impl TriviaPositionKey {
    fn of_node(node: &ast::Node) -> Self {
        Self::Node(*node)
    }

    fn of_source_node_list(nodes: ast::SourceNodeList<'_>) -> Self {
        Self::NodeList(nodes.position_key())
    }
}

pub fn new_change_tracker_writer(newline: String, indent_size: i32) -> ChangeTrackerWriter {
    // Upstream note: callers passing -1 should pass actual indent options once
    // indent-related formatting is ported.
    let indent_size = if indent_size < 0 {
        DEFAULT_INDENT_SIZE
    } else {
        indent_size
    };
    let state = Rc::new(RefCell::new(ChangeTrackerWriterState {
        text_writer: TextWriter::new_with_indent_size(newline, indent_size),
        last_non_trivia_position: 0,
        pos: HashMap::new(),
        end: HashMap::new(),
    }));
    state.borrow_mut().text_writer.clear();
    ChangeTrackerWriter { inner: state }
}

impl ChangeTrackerWriter {
    pub fn get_print_handlers(&mut self) -> PrintHandlers {
        let before_node = Rc::clone(&self.inner);
        let after_node = Rc::clone(&self.inner);
        let before_node_list = Rc::clone(&self.inner);
        let after_node_list = Rc::clone(&self.inner);
        let before_token = Rc::clone(&self.inner);
        let after_token = Rc::clone(&self.inner);
        PrintHandlers {
            on_before_emit_node: Some(Box::new(move |node_opt| {
                if let Some(node) = node_opt {
                    before_node
                        .borrow_mut()
                        .set_pos(TriviaPositionKey::of_node(node));
                }
            })),
            on_after_emit_node: Some(Box::new(move |node_opt| {
                if let Some(node) = node_opt {
                    after_node
                        .borrow_mut()
                        .set_end(TriviaPositionKey::of_node(node));
                }
            })),
            on_before_emit_node_list: Some(Box::new(move |nodes_opt| {
                if let Some(nodes) = nodes_opt {
                    before_node_list
                        .borrow_mut()
                        .set_pos(TriviaPositionKey::of_source_node_list(nodes));
                }
            })),
            on_after_emit_node_list: Some(Box::new(move |nodes_opt| {
                if let Some(nodes) = nodes_opt {
                    after_node_list
                        .borrow_mut()
                        .set_end(TriviaPositionKey::of_source_node_list(nodes));
                }
            })),
            on_before_emit_token: Some(Box::new(move |node_opt| {
                if let Some(node) = node_opt {
                    before_token
                        .borrow_mut()
                        .set_pos(TriviaPositionKey::of_node(node));
                }
            })),
            on_after_emit_token: Some(Box::new(move |node_opt| {
                if let Some(node) = node_opt {
                    after_token
                        .borrow_mut()
                        .set_end(TriviaPositionKey::of_node(node));
                }
            })),
            ..PrintHandlers::default()
        }
    }

    pub fn set_last_non_trivia_position(&mut self, s: &str, force: bool) {
        self.inner
            .borrow_mut()
            .set_last_non_trivia_position(s, force);
    }

    pub fn assign_positions_to_node(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
        factory: &mut ast::NodeFactory,
    ) -> ast::Node {
        let mut state = self.inner.borrow_mut();
        let mut traversal = AssignPositionsTraversal {
            source,
            factory,
            state: &mut state,
            import_state: ast::AstImportState::new(),
        };
        traversal.assign_positions_to_node_worker(node)
    }
}

impl ChangeTrackerWriterState {
    fn set_pos(&mut self, node: TriviaPositionKey) {
        self.pos.insert(node, self.last_non_trivia_position);
    }

    fn set_end(&mut self, node: TriviaPositionKey) {
        self.end.insert(node, self.last_non_trivia_position);
    }

    fn get_pos(&self, node: TriviaPositionKey) -> i32 {
        self.pos[&node]
    }

    fn get_end(&self, node: TriviaPositionKey) -> i32 {
        self.end[&node]
    }

    fn set_last_non_trivia_position(&mut self, s: &str, force: bool) {
        if force || scanner::skip_trivia(s, 0) != s.len() {
            self.last_non_trivia_position = self.text_writer.get_text_pos();
            // trim trailing whitespaces
            let mut pos = s.len();
            while pos > 0 {
                let mut chars = s[..pos].chars();
                let Some(r) = chars.next_back() else {
                    break;
                };
                if stringutil::is_white_space_like(r) {
                    pos -= r.len_utf8();
                } else {
                    break;
                }
            }
            self.last_non_trivia_position -= (s.len() - pos) as i32;
        }
    }

    fn try_pos(&self, node: TriviaPositionKey) -> Option<i32> {
        self.pos.get(&node).copied()
    }

    fn try_end(&self, node: TriviaPositionKey) -> Option<i32> {
        self.end.get(&node).copied()
    }
}

struct AssignPositionsTraversal<'state, 'factory, 'source> {
    source: &'source ast::AstStore,
    factory: &'factory mut ast::NodeFactory,
    state: &'state mut ChangeTrackerWriterState,
    import_state: ast::AstImportState,
}

impl AssignPositionsTraversal<'_, '_, '_> {
    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        if node.store_id() == self.factory.store().store_id() {
            self.factory.store()
        } else {
            self.source
        }
    }

    fn assign_positions_to_node_worker(&mut self, node: &ast::Node) -> ast::Node {
        let visited = self.generated_visit_each_child(node);
        let mut new_node = visited;
        if visited.store_id() != self.factory.store().store_id() {
            assert_eq!(
                visited.store_id(),
                self.source.store_id(),
                "change-tracker traversal cannot clone unrelated AST store"
            );
            new_node = self
                .factory
                .deep_clone_node_from_store(self.source, visited);
        }
        self.factory.adopt_emit_synthetic_children(new_node);
        let loc = core::TextRange::new(
            self.state.get_pos(TriviaPositionKey::of_node(node)),
            self.state.get_end(TriviaPositionKey::of_node(node)),
        );
        self.factory.place_emit_synthetic_node(new_node, loc);
        new_node
    }

    fn assign_positions_to_node_array(&mut self, nodes: ast::SourceNodeListInput) -> ast::NodeList {
        let source_nodes = nodes.resolve(self.source);
        let mut visited_nodes = Vec::with_capacity(nodes.len());
        for node in nodes.iter() {
            if let Some(visited) = self.visit_node(Some(node)) {
                visited_nodes.push(visited);
            }
        }
        let visited = self.factory.new_node_list_with_trailing_comma(
            nodes.loc(),
            nodes.range(),
            visited_nodes,
            nodes.has_trailing_comma(),
        );
        let range = self.factory.emit_node_list_range(visited);
        let loc = core::TextRange::new(
            self.state
                .get_pos(TriviaPositionKey::of_source_node_list(source_nodes)),
            self.state
                .get_end(TriviaPositionKey::of_source_node_list(source_nodes)),
        );
        let cloned_nodes = self.factory.emit_node_list_nodes(visited);
        self.factory.new_node_list(loc, range, cloned_nodes)
    }

    fn modifier_list_loc(&self, nodes: &ast::SourceModifierListInput) -> core::TextRange {
        let source_nodes = nodes.nodes();
        let first = source_nodes.first().copied();
        let last = source_nodes.last().copied();
        match (first, last) {
            (Some(first), Some(last)) => {
                let pos = self
                    .state
                    .try_pos(TriviaPositionKey::of_node(&first))
                    .unwrap_or_else(|| nodes.loc().pos());
                let end = self
                    .state
                    .try_end(TriviaPositionKey::of_node(&last))
                    .unwrap_or_else(|| nodes.loc().end());
                core::TextRange::new(pos, end)
            }
            _ => nodes.loc(),
        }
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
                let statements = self.factory.new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    Vec::<ast::Node>::new(),
                );
                return Some(self.factory.new_block(statements, true));
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
            let statements = self.factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
            );
            self.factory.new_block(statements, true)
        }
    }

    fn preserve_source_node_list_input(
        &mut self,
        nodes: &ast::SourceNodeListInput,
    ) -> ast::NodeList {
        self.import_state
            .preserve_source_node_list_input(self.source, self.factory, nodes)
    }

    fn preserve_source_raw_node_slice_input(
        &mut self,
        nodes: &ast::SourceRawNodeSliceInput,
    ) -> ast::RawNodeSlice {
        self.import_state
            .preserve_source_raw_node_slice_input(self.source, self.factory, nodes)
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for AssignPositionsTraversal<'_, '_, 'source> {
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ast::NodeFactory {
        self.factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        self.factory
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        self.import_state.preserved_node(self.factory, source)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory.store().store_id() {
            return node;
        }
        self.import_state
            .preserve_node(self.source, self.factory, node)
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        self.import_state
            .record_preserved_node(source.store_id(), self.factory, source, imported)
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        self.import_state
            .preserved_source_node_matches(self.factory, source, output)
    }

    fn source_store_for_store_id(&self, store_id: ast::StoreId) -> &ast::AstStore {
        assert_eq!(
            store_id,
            self.source.store_id(),
            "change-tracker traversal cannot resolve unrelated AST store"
        );
        self.source
    }

    fn preserved_source_node_list_input_matches(
        &self,
        source: Option<&ast::SourceNodeListInput>,
        output: Option<ast::NodeList>,
    ) -> bool {
        self.import_state.preserved_source_node_list_input_matches(
            self.source,
            self.factory,
            source,
            output,
        )
    }

    fn preserved_source_modifier_list_input_matches(
        &self,
        source: Option<&ast::SourceModifierListInput>,
        output: Option<ast::ModifierList>,
    ) -> bool {
        self.import_state
            .preserved_source_modifier_list_input_matches(self.source, self.factory, source, output)
    }

    fn preserved_source_raw_node_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawNodeSliceInput>,
        output: Option<ast::RawNodeSlice>,
    ) -> bool {
        self.import_state
            .preserved_source_raw_node_slice_input_matches(
                self.source,
                self.factory,
                source,
                output,
            )
    }

    fn preserved_source_raw_string_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawStringSliceInput>,
        output: Option<ast::RawStringSlice>,
    ) -> bool {
        self.import_state
            .preserved_source_raw_string_slice_input_matches(
                self.source,
                self.factory,
                source,
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
        assert_eq!(
            node.store_id(),
            self.source.store_id(),
            "change-tracker traversal cannot update a source file from an unrelated AST store"
        );
        if source_unchanged {
            let imported = self.preserve_node(node);
            return self.record_preserved_node(node, imported);
        }
        self.import_state.update_source_file_from_store(
            self.source,
            self.factory,
            node,
            statements.expect("source file statements cannot be removed"),
            end_of_file_token,
        )
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.assign_positions_to_node_worker(&node))
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_embedded_statement(node)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let visited = self.visit_node(node);
        self.lift_to_block_or_empty(visited)
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source>
    for AssignPositionsTraversal<'_, '_, 'source>
{
}

impl EmitTextWriter for ChangeTrackerWriter {
    fn write(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_trailing_semicolon(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_comment(&mut self, text: &str) {
        self.inner.borrow_mut().text_writer.write_comment(text)
    }

    fn write_keyword(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_keyword(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_operator(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_operator(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_punctuation(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_punctuation(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_space(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_space(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_string_literal(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_string_literal(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_parameter(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_parameter(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_property(&mut self, text: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_property(text);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_symbol(&mut self, text: &str, symbol: Option<ast::SymbolHandle>) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_symbol(text, symbol);
        inner.set_last_non_trivia_position(text, false);
    }

    fn write_line(&mut self) {
        self.inner.borrow_mut().text_writer.write_line()
    }

    fn write_line_force(&mut self, force: bool) {
        self.inner.borrow_mut().text_writer.write_line_force(force)
    }

    fn increase_indent(&mut self) {
        self.inner.borrow_mut().text_writer.increase_indent()
    }

    fn decrease_indent(&mut self) {
        self.inner.borrow_mut().text_writer.decrease_indent()
    }

    fn clear(&mut self) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.clear();
        inner.last_non_trivia_position = 0;
    }

    fn string(&self) -> String {
        self.inner.borrow().text_writer.string()
    }

    fn raw_write(&mut self, s: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.raw_write(s);
        inner.set_last_non_trivia_position(s, false);
    }

    fn write_literal(&mut self, s: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.text_writer.write_literal(s);
        inner.set_last_non_trivia_position(s, true);
    }

    fn get_text_pos(&self) -> i32 {
        self.inner.borrow().text_writer.get_text_pos()
    }

    fn get_line(&self) -> i32 {
        self.inner.borrow().text_writer.get_line()
    }

    fn get_column(&self) -> UTF16Offset {
        self.inner.borrow().text_writer.get_column()
    }

    fn get_indent(&self) -> i32 {
        self.inner.borrow().text_writer.get_indent()
    }

    fn is_at_start_of_line(&self) -> bool {
        self.inner.borrow().text_writer.is_at_start_of_line()
    }

    fn has_trailing_comment(&self) -> bool {
        self.inner.borrow().text_writer.has_trailing_comment()
    }

    fn has_trailing_whitespace(&self) -> bool {
        self.inner.borrow().text_writer.has_trailing_whitespace()
    }
}
