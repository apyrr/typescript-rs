#![forbid(unsafe_code)]

use std::ops::ControlFlow;

use ts_ast as ast;
use ts_core as core;
use ts_scanner as scanner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenInfo {
    pub node: Option<ast::Node>,
    pub kind: ast::Kind,
    pub loc: core::TextRange,
    pub parent: Option<ast::Node>,
}

impl TokenInfo {
    pub fn from_node(store: &ast::AstStore, node: ast::Node) -> Self {
        Self {
            node: Some(node),
            kind: store.kind(node),
            loc: store.loc(node),
            parent: store.parent(node),
        }
    }

    fn scanned(
        kind: ast::Kind,
        loc: core::TextRange,
        parent: impl Into<Option<ast::Node>>,
    ) -> Self {
        Self {
            node: None,
            kind,
            loc,
            parent: parent.into(),
        }
    }
}

pub fn get_touching_property_name(
    source_file: &ast::SourceFile,
    position: i32,
) -> Option<ast::Node> {
    let store = source_file.store();
    let include_preceding_token_at_end_position = |node: ast::Node| {
        ast::is_property_name_literal(store, node)
            || ast::is_keyword_kind(store.kind(node))
            || ast::is_private_identifier(store, node)
    };
    get_token_at_position_worker(
        source_file,
        position,
        false, /*allowPositionInLeadingTrivia*/
        Some(&include_preceding_token_at_end_position),
    )
}

pub fn get_touching_token(source_file: &ast::SourceFile, position: i32) -> Option<ast::Node> {
    get_token_at_position_worker(
        source_file,
        position,
        false, /*allowPositionInLeadingTrivia*/
        None,
    )
}

pub fn get_token_at_position(source_file: &ast::SourceFile, position: i32) -> Option<ast::Node> {
    get_token_at_position_worker(
        source_file,
        position,
        true, /*allowPositionInLeadingTrivia*/
        None,
    )
}

pub fn get_token_at_position_info(
    source_file: &ast::SourceFile,
    position: i32,
) -> Option<TokenInfo> {
    // getTokenAtPosition returns a token at the given position in the source file.
    // The token can be a real node in the AST, or a synthesized token constructed
    // with information from the scanner.
    let node = get_token_at_position(source_file, position)?;
    let store = source_file.store();
    if ast::is_token_kind(store.kind(node)) {
        return Some(TokenInfo::from_node(store, node));
    }

    let loc = store.loc(node);
    let scan_start = position.max(loc.pos()).min(loc.end()).max(0) as usize;
    let mut scanner = scanner::get_scanner_for_source_file(source_file, scan_start);
    let token = scan_navigation_token(&mut scanner, store, node);
    let token_full_start = scanner.token_full_start();
    let token_end = scanner.token_end();
    if token_full_start <= position && position < token_end && ast::is_token_kind(token) {
        return Some(TokenInfo::scanned(
            token,
            core::new_text_range(token_full_start, token_end),
            node,
        ));
    }

    Some(TokenInfo::from_node(store, node))
}

fn get_token_at_position_worker(
    source_file: &ast::SourceFile,
    position: i32,
    allow_position_in_leading_trivia: bool,
    include_preceding_token_at_end_position: Option<&dyn Fn(ast::Node) -> bool>,
) -> Option<ast::Node> {
    // getTokenAtPosition returns a token at the given position in the source file.
    // The token can be a real node in the AST, or a synthesized token constructed
    // with information from the scanner. Synthesized tokens are only created when
    // needed, and they are stored in the source file's token cache such that multiple
    // calls to getTokenAtPosition with the same position will return the same object
    // in memory. If there is no token at the given position (possible when
    // `allowPositionInLeadingTrivia` is false), the lowest node that encloses the
    // position is returned.
    get_token_at_position_from_node(
        source_file,
        source_file.as_node(),
        position,
        allow_position_in_leading_trivia,
        include_preceding_token_at_end_position,
    )
}

fn get_token_at_position_from_node(
    source_file: &ast::SourceFile,
    node: ast::Node,
    position: i32,
    allow_position_in_leading_trivia: bool,
    include_preceding_token_at_end_position: Option<&dyn Fn(ast::Node) -> bool>,
) -> Option<ast::Node> {
    if node.store_id() != source_file.store().store_id() {
        return None;
    }
    let store = source_file.store();
    if store.flags(node).intersects(ast::NodeFlags::REPARSED) {
        return None;
    }
    let loc = store.loc(node);

    // A node "contains" the position if position < end, except nodes at the file end
    // treat end as inclusive (there's nowhere else to look). This applies to the EOF
    // token itself, and to JSDoc nodes reaching EOF (e.g. unterminated JSDoc comments).
    if loc.end() < position
        || (loc.end() == position
            && store.kind(node) != ast::Kind::EndOfFile
            && store.kind(node) != ast::Kind::SourceFile
            && include_preceding_token_at_end_position.is_none_or(|include| !include(node)))
    {
        return None;
    }
    if get_position(node, source_file, allow_position_in_leading_trivia) > position {
        return None;
    }

    if ast::is_token_kind(store.kind(node)) {
        return Some(node);
    }

    // We zero in on the node that contains the target position by visiting each
    // child of the current node.
    let mut result = None;
    let _ = for_each_navigation_child(store, node, |child, _| {
        if let Some(found) = get_token_at_position_from_node(
            source_file,
            child,
            position,
            allow_position_in_leading_trivia,
            include_preceding_token_at_end_position,
        ) {
            result = Some(found);
        }
        ControlFlow::Continue(())
    });
    result.or_else(|| (store.kind(node) != ast::Kind::EndOfFile).then_some(node))
}

fn get_position(
    node: ast::Node,
    source_file: &ast::SourceFile,
    allow_position_in_leading_trivia: bool,
) -> i32 {
    if allow_position_in_leading_trivia {
        return source_file.store().loc(node).pos();
    }
    scanner::get_token_pos_of_node(&node, source_file, true /*includeJSDoc*/) as i32
}

fn for_each_navigation_child<F>(
    store: &ast::AstStore,
    node: ast::Node,
    mut visit: F,
) -> ControlFlow<()>
where
    F: FnMut(ast::Node, core::TextRange) -> ControlFlow<()>,
{
    store.for_each_child_node_source_span(node, |span| {
        let Some(child) = span.node() else {
            return ControlFlow::Continue(());
        };
        if store.flags(child).intersects(ast::NodeFlags::REPARSED) {
            return ControlFlow::Continue(());
        }
        visit(child, span.loc().unwrap_or_else(|| store.loc(child)))
    })
}

fn find_function_declaration_typed_child_of_kind(
    store: &ast::AstStore,
    node: ast::Node,
    kind: ast::Kind,
) -> Option<ast::Node> {
    let function = store.ast_ref::<ast::FunctionDeclarationView>(node)?;
    function
        .find_modifiers::<ast::AstTokenView>(|child| child.kind() == kind)
        .map(|child| child.node())
        .or_else(|| {
            function
                .asterisk_token::<ast::AstTokenView>()
                .filter(|child| child.kind() == kind)
                .map(|child| child.node())
        })
        .or_else(|| {
            function
                .name::<ast::AstNameView>()
                .filter(|child| child.kind() == kind)
                .map(|child| child.node())
        })
        .or_else(|| {
            function
                .find_type_parameters::<ast::TypeParameterDeclarationView>(|child| {
                    child.kind() == kind
                })
                .map(|child| child.node())
        })
        .or_else(|| {
            function
                .find_parameters::<ast::ParameterDeclarationView>(|child| child.kind() == kind)
                .map(|child| child.node())
        })
        .or_else(|| {
            function
                .r#type::<ast::AstTypeNodeView>()
                .filter(|child| child.kind() == kind)
                .map(|child| child.node())
        })
        .or_else(|| {
            function
                .full_signature_node()
                .filter(|child| store.kind(*child) == kind)
        })
        .or_else(|| {
            function
                .body::<ast::BlockView>()
                .filter(|child| child.kind() == kind)
                .map(|child| child.node())
        })
}

fn find_source_file_typed_child_of_kind(
    source_file: ast::SourceFileView<'_>,
    kind: ast::Kind,
) -> Option<ast::Node> {
    source_file
        .find_statements::<ast::AstStatementView>(|child| child.kind() == kind)
        .map(|child| child.node())
        .or_else(|| {
            source_file
                .end_of_file_token_view::<ast::AstTokenView>()
                .filter(|child| child.kind() == kind)
                .map(|child| child.node())
        })
}

fn find_typed_navigation_child_of_kind(
    store: &ast::AstStore,
    node: ast::Node,
    kind: ast::Kind,
) -> Option<ast::Node> {
    match store.kind(node) {
        ast::Kind::SourceFile => {
            find_source_file_typed_child_of_kind(store.source_file_view(node), kind)
        }
        ast::Kind::FunctionDeclaration => {
            find_function_declaration_typed_child_of_kind(store, node, kind)
        }
        _ => None,
    }
}

// Finds the leftmost token satisfying `position < token.End()`.
// If the leftmost token satisfying `position < token.End()` is invalid, or if position
// is in the trivia of that leftmost token,
// we will find the rightmost valid token with `token.End() <= position`.
pub fn find_preceding_token<P>(source_file: &ast::SourceFile, position: P) -> Option<ast::Node>
where
    P: TryInto<i32>,
{
    find_preceding_token_ex(source_file, position, None)
}

pub fn find_preceding_token_info<P>(source_file: &ast::SourceFile, position: P) -> Option<TokenInfo>
where
    P: TryInto<i32>,
{
    find_preceding_token_ex_info(source_file, position, None)
}

pub fn find_preceding_token_ex<P>(
    source_file: &ast::SourceFile,
    position: P,
    start_node: Option<ast::Node>,
) -> Option<ast::Node>
where
    P: TryInto<i32>,
{
    let position = position.try_into().ok()?;
    let node = start_node.unwrap_or_else(|| source_file.as_node());
    let result = find_preceding_token_in_node(source_file, position, node);
    if result.is_some_and(|node| is_whitespace_only_jsx_text(source_file.store(), node)) {
        panic!("Expected result to be a non-whitespace token.");
    }
    result
}

pub fn find_preceding_token_ex_info<P>(
    source_file: &ast::SourceFile,
    position: P,
    start_node: Option<ast::Node>,
) -> Option<TokenInfo>
where
    P: TryInto<i32>,
{
    let position = position.try_into().ok()?;
    let node = start_node.unwrap_or_else(|| source_file.as_node());
    let result = find_preceding_token_info_in_node(source_file, position, node);
    if result.is_some_and(|token| {
        token
            .node
            .is_some_and(|node| is_whitespace_only_jsx_text(source_file.store(), node))
    }) {
        panic!("Expected result to be a non-whitespace token.");
    }
    result
}

fn find_preceding_token_in_node(
    source_file: &ast::SourceFile,
    position: i32,
    node: ast::Node,
) -> Option<ast::Node> {
    let store = source_file.store();
    if is_non_whitespace_token(store, node) && store.kind(node) != ast::Kind::EndOfFile {
        return Some(node);
    }

    // `foundChild` is the leftmost node that contains the target position.
    // `prevChild` is the last visited child of the current node.
    let mut found_child = None;
    let mut prev_child = None;
    let _ = for_each_navigation_child(store, node, |child, child_loc| {
        if found_child.is_some() {
            // We cannot abort visiting children, so once the desired child is found, we do nothing.
            return ControlFlow::Continue(());
        }
        if position < child_loc.end()
            && prev_child.is_none_or(|prev_child| store.loc(prev_child).end() <= position)
        {
            found_child = Some(child);
        } else {
            prev_child = Some(child);
        }
        ControlFlow::Continue(())
    });

    if let Some(found_child) = found_child {
        // Note that the span of a node's tokens is [getStartOfNode(node, ...), node.end).
        // Given that `position < child.end` and child has constituent tokens, we distinguish these cases:
        // 1) `position` precedes `child`'s tokens or `child` has no tokens (ie: in a comment or whitespace preceding `child`):
        // we need to find the last token in a previous child node or child tokens.
        // 2) `position` is within the same span: we recurse on `child`.
        let start = get_start_of_node_with_include_jsdoc(found_child, source_file, true);
        let look_in_previous_child =
            start >= position || !is_valid_preceding_node(found_child, source_file);
        if look_in_previous_child {
            if position >= store.loc(found_child).pos() {
                return find_rightmost_valid_token(
                    store.loc(found_child).pos(),
                    source_file,
                    node,
                    -1, /*position*/
                );
            }
            // Answer is in tokens between two visited children.
            return find_rightmost_valid_token(
                store.loc(found_child).pos(),
                source_file,
                node,
                position,
            );
        }
        // position is in [foundChild.getStart(), foundChild.End): recur.
        return find_preceding_token_in_node(source_file, position, found_child);
    }

    // We have two cases here: either the position is at the end of the file,
    // or the desired token is in the unvisited trailing tokens of the current node.
    let node_end = store.loc(node).end();
    if position >= node_end {
        find_rightmost_valid_token(node_end, source_file, node, -1 /*position*/)
    } else {
        find_rightmost_valid_token(node_end, source_file, node, position)
    }
}

fn find_preceding_token_info_in_node(
    source_file: &ast::SourceFile,
    position: i32,
    node: ast::Node,
) -> Option<TokenInfo> {
    let store = source_file.store();
    if is_non_whitespace_token(store, node) && store.kind(node) != ast::Kind::EndOfFile {
        return Some(TokenInfo::from_node(store, node));
    }

    let mut found_child = None;
    let mut prev_child = None;
    let _ = for_each_navigation_child(store, node, |child, child_loc| {
        if found_child.is_some() {
            return ControlFlow::Continue(());
        }
        if position < child_loc.end()
            && prev_child.is_none_or(|prev_child| store.loc(prev_child).end() <= position)
        {
            found_child = Some(child);
        } else {
            prev_child = Some(child);
        }
        ControlFlow::Continue(())
    });

    if let Some(found_child) = found_child {
        let start = get_start_of_node_with_include_jsdoc(found_child, source_file, true);
        let look_in_previous_child =
            start >= position || !is_valid_preceding_node(found_child, source_file);
        if look_in_previous_child {
            if position >= store.loc(found_child).pos() {
                return find_rightmost_valid_token_info(
                    store.loc(found_child).pos(),
                    source_file,
                    node,
                    -1, /*position*/
                );
            }
            return find_rightmost_valid_token_info(
                store.loc(found_child).pos(),
                source_file,
                node,
                position,
            );
        }
        return find_preceding_token_info_in_node(source_file, position, found_child);
    }

    let node_end = store.loc(node).end();
    if position >= node_end {
        find_rightmost_valid_token_info(node_end, source_file, node, -1 /*position*/)
    } else {
        find_rightmost_valid_token_info(node_end, source_file, node, position)
    }
}

pub fn find_next_token(
    previous_token: ast::Node,
    parent: ast::Node,
    file: &ast::SourceFile,
) -> Option<ast::Node> {
    find_next_token_in_node(previous_token, parent, file)
}

fn find_next_token_in_node(
    previous_token: ast::Node,
    parent: ast::Node,
    file: &ast::SourceFile,
) -> Option<ast::Node> {
    let store = file.store();
    if ast::is_token_kind(store.kind(parent))
        && store.loc(parent).pos() == store.loc(previous_token).end()
    {
        // this is token that starts at the end of previous token - return it
        return Some(parent);
    }

    // Node that contains `previousToken` or occurs immediately after it.
    let previous_end = store.loc(previous_token).end();
    let mut found_node = None;
    let _ = for_each_navigation_child(store, parent, |child, child_loc| {
        if found_node.is_some() {
            return ControlFlow::Continue(());
        }
        if child_loc.pos() <= previous_end && child_loc.end() > previous_end {
            found_node = Some(child);
        } else if child_loc.pos() >= previous_end {
            found_node = Some(child);
        }
        ControlFlow::Continue(())
    });

    // Cases:
    // 1. no answer exists
    // 2. answer is an unvisited token
    // 3. answer is in the visited found node

    // Case 3: look for the next token inside the found node.
    found_node.and_then(|found_node| find_leftmost_token(found_node, file))
}

pub fn find_child_of_kind(
    node: ast::Node,
    kind: ast::Kind,
    source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    find_child_of_kind_info(node, kind, source_file).and_then(|child| child.node)
}

pub fn find_child_of_kind_info(
    node: ast::Node,
    kind: ast::Kind,
    source_file: &ast::SourceFile,
) -> Option<TokenInfo> {
    let store = source_file.store();
    if store.kind(node) == kind {
        return Some(TokenInfo::from_node(store, node));
    }
    if let Some(child) = find_typed_navigation_child_of_kind(store, node, kind) {
        return Some(TokenInfo::from_node(store, child));
    }

    let containing_loc = store.loc(node);
    let mut last_node_pos = containing_loc.pos();
    let mut scan = scanner::get_scanner_for_source_file(source_file, last_node_pos.max(0) as usize);
    let mut found_child = None;
    let _ = for_each_navigation_child(store, node, |child, child_loc| {
        if let Some(token) = find_scanned_child_token_of_kind(
            source_file,
            node,
            &mut scan,
            &mut last_node_pos,
            child_loc.pos(),
            kind,
        ) {
            found_child = Some(token);
            return ControlFlow::Break(());
        }
        if store.kind(child) == kind {
            found_child = Some(TokenInfo::from_node(store, child));
            return ControlFlow::Break(());
        }

        last_node_pos = child_loc.end();
        scan.reset_pos(last_node_pos);
        scan.scan();
        ControlFlow::Continue(())
    });

    found_child.or_else(|| {
        find_scanned_child_token_of_kind(
            source_file,
            node,
            &mut scan,
            &mut last_node_pos,
            containing_loc.end(),
            kind,
        )
    })
}

pub fn has_child_of_kind(node: ast::Node, kind: ast::Kind, source_file: &ast::SourceFile) -> bool {
    find_child_of_kind_info(node, kind, source_file).is_some()
}

pub fn get_start_of_token_info(token: TokenInfo, file: &ast::SourceFile) -> i32 {
    if let Some(node) = token.node {
        return get_start_of_node(node, file);
    }
    scanner::skip_trivia(file.text(), token.loc.pos().max(0) as usize) as i32
}

pub fn range_from_token_info(token: TokenInfo, file: &ast::SourceFile) -> core::TextRange {
    core::new_text_range(get_start_of_token_info(token, file), token.loc.end())
}

pub fn get_start_of_node(node: ast::Node, file: &ast::SourceFile) -> i32 {
    get_start_of_node_with_include_jsdoc(node, file, false /*includeJSDoc*/)
}

fn get_start_of_node_with_include_jsdoc(
    node: ast::Node,
    file: &ast::SourceFile,
    include_jsdoc: bool,
) -> i32 {
    scanner::get_token_pos_of_node(&node, file, include_jsdoc) as i32
}

fn find_rightmost_valid_token(
    end_pos: i32,
    source_file: &ast::SourceFile,
    containing_node: ast::Node,
    position: i32,
) -> Option<ast::Node> {
    let position = if position == -1 {
        source_file.store().loc(containing_node).end()
    } else {
        position
    };
    find_rightmost_valid_token_in_node(
        end_pos,
        source_file,
        containing_node,
        containing_node,
        position,
    )
}

fn find_rightmost_valid_token_in_node(
    end_pos: i32,
    source_file: &ast::SourceFile,
    containing_node: ast::Node,
    node: ast::Node,
    position: i32,
) -> Option<ast::Node> {
    let store = source_file.store();
    if is_non_whitespace_token(store, node) {
        return Some(node);
    }

    let mut rightmost_valid_node = None;
    let mut has_children = false;
    let _ = for_each_navigation_child(store, node, |child, child_loc| {
        has_children = true;
        if child_loc.end() > end_pos
            || get_start_of_node_with_include_jsdoc(child, source_file, true /*includeJSDoc*/)
                >= position
        {
            return ControlFlow::Continue(());
        }
        if is_valid_preceding_node(child, source_file) {
            rightmost_valid_node = Some(child);
        }
        ControlFlow::Continue(())
    });

    // Three cases:
    // 1. The answer is a token of `rightmostValidNode`.
    // 2. The answer is one of the unvisited tokens that occur after the rightmost valid node.
    // 3. The current node is a childless, token-less node. The answer is the current node.

    // Case 3: childless node.
    if !has_children {
        if node != containing_node {
            return Some(node);
        }
        return None;
    }
    // Case 1: recur on rightmostValidNode.
    rightmost_valid_node.and_then(|rightmost_valid_node| {
        find_rightmost_valid_token_in_node(
            store.loc(rightmost_valid_node).end(),
            source_file,
            containing_node,
            rightmost_valid_node,
            position,
        )
    })
}

fn find_rightmost_valid_token_info(
    end_pos: i32,
    source_file: &ast::SourceFile,
    containing_node: ast::Node,
    position: i32,
) -> Option<TokenInfo> {
    let position = if position == -1 {
        source_file.store().loc(containing_node).end()
    } else {
        position
    };
    find_rightmost_valid_token_info_in_node(
        end_pos,
        source_file,
        containing_node,
        containing_node,
        position,
    )
}

// Looks for rightmost valid token in the range [startPos, endPos).
// If position is >= 0, looks for rightmost valid token that precedes or touches that position.
fn find_rightmost_valid_token_info_in_node(
    end_pos: i32,
    source_file: &ast::SourceFile,
    containing_node: ast::Node,
    node: ast::Node,
    position: i32,
) -> Option<TokenInfo> {
    let store = source_file.store();
    if is_non_whitespace_token(store, node) {
        return Some(TokenInfo::from_node(store, node));
    }

    let mut rightmost_valid_node = None;
    // Nodes after the last valid node.
    let mut rightmost_visited_nodes = Vec::new();
    let mut has_children = false;
    let _ = for_each_navigation_child(store, node, |child, child_loc| {
        has_children = true;
        // Node is synthetic or out of the desired range: don't visit it.
        if child_loc.end() > end_pos
            || get_start_of_node_with_include_jsdoc(child, source_file, true /*includeJSDoc*/)
                >= position
        {
            return ControlFlow::Continue(());
        }
        rightmost_visited_nodes.push(child);
        if is_valid_preceding_node(child, source_file) {
            rightmost_valid_node = Some(child);
            rightmost_visited_nodes.clear();
        }
        ControlFlow::Continue(())
    });

    let mut start_pos = rightmost_valid_node
        .map(|node| store.loc(node).end())
        .unwrap_or_else(|| store.loc(node).pos());
    let mut scanner = scanner::get_scanner_for_source_file(source_file, start_pos.max(0) as usize);
    let mut tokens = Vec::new();

    // Three cases:
    // 1. The answer is a token of `rightmostValidNode`.
    // 2. The answer is one of the unvisited tokens that occur after the rightmost valid node.
    // 3. The current node is a childless, token-less node. The answer is the current node.

    // Case 2: Look at unvisited trailing tokens that occur in between the rightmost visited nodes.
    for visited_node in rightmost_visited_nodes {
        let visited_loc = store.loc(visited_node);
        // Trailing tokens that occur before this node.
        collect_scanned_tokens(
            source_file,
            node,
            &mut scanner,
            &mut start_pos,
            visited_loc.pos().min(position),
            &mut tokens,
        );
        start_pos = visited_loc.end();
        scanner.reset_pos(start_pos);
        scanner.scan();
    }

    // Trailing tokens after last visited node.
    collect_scanned_tokens(
        source_file,
        node,
        &mut scanner,
        &mut start_pos,
        end_pos.min(position),
        &mut tokens,
    );

    if let Some(token) = tokens.into_iter().rev().next() {
        return Some(token);
    }

    // Case 3: childless node.
    if !has_children {
        if node != containing_node {
            return Some(TokenInfo::from_node(store, node));
        }
        return None;
    }

    // Case 1: recur on rightmostValidNode.
    rightmost_valid_node.and_then(|rightmost_valid_node| {
        find_rightmost_valid_token_info_in_node(
            store.loc(rightmost_valid_node).end(),
            source_file,
            containing_node,
            rightmost_valid_node,
            position,
        )
    })
}

fn collect_scanned_tokens(
    source_file: &ast::SourceFile,
    parent: ast::Node,
    scanner: &mut scanner::Scanner,
    start_pos: &mut i32,
    end_pos: i32,
    tokens: &mut Vec<TokenInfo>,
) {
    while *start_pos < end_pos {
        let token = scan_navigation_token(scanner, source_file.store(), parent);
        let token_start = scanner.token_start();
        if token_start >= end_pos {
            break;
        }
        let token_full_start = scanner.token_full_start();
        let token_end = scanner.token_end();
        if token_end <= *start_pos {
            break;
        }
        tokens.push(TokenInfo::scanned(
            token,
            core::new_text_range(token_full_start, token_end),
            parent,
        ));
        *start_pos = token_end;
        scanner.scan();
    }
}

fn find_scanned_child_token_of_kind(
    source_file: &ast::SourceFile,
    parent: ast::Node,
    scan: &mut scanner::Scanner,
    start_pos: &mut i32,
    end_pos: i32,
    kind: ast::Kind,
) -> Option<TokenInfo> {
    while *start_pos < end_pos {
        let token = scan_navigation_token(scan, source_file.store(), parent);
        let token_start = scan.token_start();
        if token_start >= end_pos {
            break;
        }
        let token_full_start = scan.token_full_start();
        let token_end = scan.token_end();
        if token_end <= *start_pos {
            break;
        }
        if token == kind {
            return Some(TokenInfo::scanned(
                token,
                core::new_text_range(token_full_start, token_end),
                parent,
            ));
        }
        *start_pos = token_end;
        scan.scan();
    }
    None
}

fn scan_navigation_token(
    scanner: &mut scanner::Scanner,
    store: &ast::AstStore,
    containing_node: ast::Node,
) -> ast::Kind {
    let token = scanner.token();
    if token == ast::Kind::LessThanLessThanToken && is_jsx_child(store, containing_node) {
        scanner.re_scan_jsx_token(true /*allowMultilineJsxText*/)
    } else {
        token
    }
}

fn is_jsx_child(store: &ast::AstStore, node: ast::Node) -> bool {
    matches!(
        store.kind(node),
        ast::Kind::JsxElement
            | ast::Kind::JsxExpression
            | ast::Kind::JsxSelfClosingElement
            | ast::Kind::JsxText
            | ast::Kind::JsxFragment
    )
}

fn find_leftmost_token(node: ast::Node, source_file: &ast::SourceFile) -> Option<ast::Node> {
    let store = source_file.store();
    if ast::is_token_kind(store.kind(node)) && store.kind(node) != ast::Kind::EndOfFile {
        return Some(node);
    }
    let mut result = None;
    let _ = for_each_navigation_child(store, node, |child, _| {
        if result.is_none() {
            result = find_leftmost_token(child, source_file);
        }
        ControlFlow::Continue(())
    });
    result
}

fn is_non_whitespace_token(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_token_kind(store.kind(node)) && !is_whitespace_only_jsx_text(store, node)
}

fn is_whitespace_only_jsx_text(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_jsx_text_all_white_spaces(store, node)
}

fn is_valid_preceding_node(node: ast::Node, source_file: &ast::SourceFile) -> bool {
    let store = source_file.store();
    if store.kind(node) == ast::Kind::EndOfFile {
        return false;
    }
    let start =
        get_start_of_node_with_include_jsdoc(node, source_file, false /*includeJSDoc*/);
    let width = store.loc(node).end() - start;
    !(is_whitespace_only_jsx_text(store, node) || width == 0)
}

pub mod tokens {
    pub use crate::{
        TokenInfo, find_child_of_kind, find_child_of_kind_info, find_next_token,
        find_preceding_token, find_preceding_token_ex, find_preceding_token_ex_info,
        find_preceding_token_info, get_start_of_node, get_start_of_token_info,
        get_token_at_position, get_touching_property_name, get_touching_token, has_child_of_kind,
        range_from_token_info,
    };
}

#[cfg(test)]
mod tests {
    use ts_ast as ast;
    use ts_core as core;
    use ts_parser as parser;

    fn parse_ts(source: &str) -> ast::SourceFile {
        parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/file.ts".to_string(),
                path: "/file.ts".into(),
                external_module_indicator_options: Default::default(),
            },
            source.to_string(),
            core::ScriptKind::TS,
        )
    }

    fn first_function_declaration(file: &ast::SourceFile) -> ast::Node {
        super::find_child_of_kind(file.as_node(), ast::Kind::FunctionDeclaration, file)
            .expect("expected function declaration")
    }

    #[test]
    fn find_child_of_kind_should_find_source_file_statement_list_child() {
        let file = parse_ts("let x = 1;");

        let statement =
            super::find_child_of_kind(file.as_node(), ast::Kind::VariableStatement, &file)
                .expect("expected statement list child");

        assert_eq!(file.store().kind(statement), ast::Kind::VariableStatement);
    }

    #[test]
    fn find_child_of_kind_should_find_source_file_eof_through_typed_root_helper() {
        let file = parse_ts("let x = 1;");

        let eof = super::find_child_of_kind(file.as_node(), ast::Kind::EndOfFile, &file)
            .expect("expected EOF child");

        assert_eq!(file.store().kind(eof), ast::Kind::EndOfFile);
    }

    #[test]
    fn find_child_of_kind_should_find_function_name_through_typed_field_helper() {
        let file = parse_ts("export function f<T>(value: string): string { return value; }");
        let function = first_function_declaration(&file);

        let name = super::find_child_of_kind(function, ast::Kind::Identifier, &file)
            .expect("expected function name");

        assert_eq!(file.store().text(name), "f");
    }

    #[test]
    fn find_child_of_kind_should_find_function_parameter_through_typed_list_helper() {
        let file = parse_ts("function f(value: string): string { return value; }");
        let function = first_function_declaration(&file);

        let parameter = super::find_child_of_kind(function, ast::Kind::Parameter, &file)
            .expect("expected function parameter");

        assert_eq!(file.store().kind(parameter), ast::Kind::Parameter);
    }

    #[test]
    fn find_child_of_kind_info_should_scan_keyword_before_first_child() {
        let file = parse_ts("import { x } from \"m\";");
        let statement =
            super::find_child_of_kind(file.as_node(), ast::Kind::ImportDeclaration, &file)
                .expect("expected import declaration");

        let token = super::find_child_of_kind_info(statement, ast::Kind::ImportKeyword, &file)
            .expect("expected import keyword");

        assert_eq!(token.kind, ast::Kind::ImportKeyword);
        assert_eq!(token.node, None);
        assert_eq!(
            super::range_from_token_info(token, &file),
            core::new_text_range(0, 6)
        );
    }

    #[test]
    fn find_child_of_kind_info_should_scan_punctuation_between_children() {
        let file = parse_ts("f(a, b);");
        let statement =
            super::find_child_of_kind(file.as_node(), ast::Kind::ExpressionStatement, &file)
                .expect("expected expression statement");
        let call = super::find_child_of_kind(statement, ast::Kind::CallExpression, &file)
            .expect("expected call expression");

        let token = super::find_child_of_kind_info(call, ast::Kind::OpenParenToken, &file)
            .expect("expected open paren");

        assert_eq!(token.kind, ast::Kind::OpenParenToken);
        assert_eq!(
            file.text()[token.loc.pos() as usize..token.loc.end() as usize].trim(),
            "("
        );
    }

    #[test]
    fn find_preceding_token_info_scans_dot_at_eof_after_incomplete_property_access() {
        let text = "namespace testModule { export var foo = 1; }\n@\ntestModule.";
        let file = parse_ts(text);

        let token = super::find_preceding_token_info(&file, text.len() as i32)
            .expect("expected preceding token");

        assert_eq!(token.kind, ast::Kind::DotToken);
        assert_eq!(
            token.parent.map(|parent| file.store().kind(parent)),
            Some(ast::Kind::PropertyAccessExpression)
        );
    }

    #[test]
    fn find_preceding_token_info_scans_identifier_at_eof_after_incomplete_object_literal() {
        let text = "var person: {name:string; id: number} = { n";
        let file = parse_ts(text);

        let token = super::find_preceding_token_info(&file, text.len() as i32)
            .expect("expected preceding token");

        assert_eq!(token.kind, ast::Kind::Identifier);
        assert_eq!(
            text[token.loc.pos() as usize..token.loc.end() as usize].trim_start(),
            "n"
        );
        assert_eq!(
            token.parent.map(|parent| file.store().kind(parent)),
            Some(ast::Kind::ShorthandPropertyAssignment)
        );
    }
}
