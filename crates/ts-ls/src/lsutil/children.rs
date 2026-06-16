use ts_ast as ast;
use ts_core as core;
use ts_scanner as scanner;

#[derive(Clone, Copy)]
pub struct TokenInfo {
    pub node: Option<ast::Node>,
    pub kind: ast::Kind,
    pub loc: core::TextRange,
}

impl TokenInfo {
    fn from_node(store: &ast::AstStore, node: ast::Node) -> Self {
        Self {
            node: Some(node),
            kind: store.kind(node),
            loc: store.loc(node),
        }
    }

    pub(crate) fn matches_node(&self, store: &ast::AstStore, node: ast::Node) -> bool {
        self.node
            .as_ref()
            .is_some_and(|token_node| *token_node == node)
            || (self.kind == store.kind(node) && self.loc == store.loc(node))
    }
}

// Replaces last(node.getChildren(sourceFile))
pub(crate) fn get_last_child(node: ast::Node, source_file: &ast::SourceFile) -> Option<ast::Node> {
    get_last_visited_child(node, source_file)
}

pub(crate) fn get_last_child_info(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Option<TokenInfo> {
    let store = source_file.store();
    let last_child_node = get_last_visited_child(node, source_file);
    let token_start_pos =
        last_child_node.map_or_else(|| store.loc(node).pos(), |node| store.loc(node).end());
    let node_end = store.loc(node).end();
    let mut scanner =
        scanner::get_scanner_for_source_file(source_file, token_start_pos.max(0) as usize);
    let mut start_pos = token_start_pos;
    let mut last_token = None;
    while start_pos < node_end {
        let token_end = scanner.token_end();
        if token_end <= start_pos {
            break;
        }
        last_token = Some(TokenInfo {
            node: None,
            kind: scanner.token(),
            loc: core::new_text_range(scanner.token_full_start(), token_end),
        });
        start_pos = token_end;
        scanner.scan();
    }

    last_token.or_else(|| last_child_node.map(|node| TokenInfo::from_node(store, node)))
}

pub(crate) fn get_last_token_info(
    node: Option<ast::Node>,
    source_file: &ast::SourceFile,
) -> Option<TokenInfo> {
    let node = node?;
    let store = source_file.store();
    if ast::is_token_kind(store.kind(node)) || ast::is_identifier(store, node) {
        return None;
    }

    assert_has_real_position(node, source_file);

    let last_child = get_last_child_info(node, source_file)?;
    if last_child.kind < ast::Kind::FirstNode {
        return Some(last_child);
    }

    last_child
        .node
        .and_then(|last_child| get_last_token_info(Some(last_child), source_file))
}

pub(crate) fn get_last_visited_child(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    let mut last_child: Option<ast::Node> = None;
    let _ = source_file.store().for_each_present_child(node, |n| {
        if !source_file
            .store()
            .flags(n)
            .contains(ast::NodeFlags::REPARSED)
        {
            last_child = Some(n);
        }
        std::ops::ControlFlow::Continue(())
    });
    last_child
}

pub(crate) fn get_first_token_info(
    node: Option<ast::Node>,
    source_file: &ast::SourceFile,
) -> Option<TokenInfo> {
    let node = node?;
    let store = source_file.store();
    if ast::is_identifier(store, node) || ast::is_token_kind(store.kind(node)) {
        return None;
    }

    assert_has_real_position(node, source_file);

    let mut first_child: Option<ast::Node> = None;
    let _ = store.for_each_present_child(node, |n| {
        if store.flags(n).contains(ast::NodeFlags::REPARSED) {
            return std::ops::ControlFlow::Continue(());
        }
        first_child = Some(n);
        std::ops::ControlFlow::Break(())
    });

    let token_end_position =
        first_child.map_or_else(|| store.loc(node).end(), |child| store.loc(child).pos());
    if store.loc(node).pos() < token_end_position {
        let scanner = scanner::get_scanner_for_source_file(
            source_file,
            store.loc(node).pos().max(0) as usize,
        );
        let token_end = scanner.token_end();
        if token_end > store.loc(node).pos() {
            return Some(TokenInfo {
                node: None,
                kind: scanner.token(),
                loc: core::new_text_range(scanner.token_full_start(), token_end),
            });
        }
    }

    let first_child = first_child?;
    if store.kind(first_child) < ast::Kind::FirstNode {
        return Some(TokenInfo::from_node(store, first_child));
    }

    get_first_token_info(Some(first_child), source_file)
}

pub(crate) fn assert_has_real_position(node: ast::Node, source_file: &ast::SourceFile) {
    let loc = source_file.store().loc(node);
    if ast::position_is_synthesized(loc.pos()) || ast::position_is_synthesized(loc.end()) {
        panic!("Node must have a real position for this operation.");
    }
}
