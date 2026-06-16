use std::cell::RefCell;
use std::cmp;
use std::ops::ControlFlow;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_scanner as scanner;

use crate::indent::{
    argument_starts_on_same_line_as_previous_argument,
    child_is_unindented_branch_of_conditional_expression,
    child_starts_on_the_same_line_with_else_in_if_statement,
    find_first_non_whitespace_character_and_column, node_will_indent_child,
    should_indent_child_node,
};
use crate::scanner::{FormattingScanner, TextRangeWithKind, TokenInfo, new_text_range_with_kind};
use crate::util::{
    get_close_token_for_open_token, get_open_token_for_list_input, is_grammar_error,
};
use crate::{
    FormatContext, FormatRequestKind, FormattingContext, RuleAction, RuleFlags,
    find_first_non_whitespace_column, get_format_code_settings_from_context,
    get_line_start_position_for_position, get_new_line_or_default_from_context, get_rules, lsutil,
    new_formatting_context, with_token_start,
};

#[derive(Clone)]
enum VisitEachChildItem {
    Node(ast::Node),
    NodeList(ast::SourceNodeListInput),
    ModifierList(ast::SourceModifierListInput),
}

/** find node that fully contains given text range */
pub fn find_enclosing_node(r: core::TextRange, source_file: &ast::SourceFile) -> ast::Node {
    fn find(n: &ast::Node, r: core::TextRange, source_file: &ast::SourceFile) -> ast::Node {
        let mut candidate: Option<ast::Node> = None;
        let store = source_file.store();
        let _ = store.for_each_present_child(*n, |c| {
            if store.flags(c).intersects(ast::NodeFlags::REPARSED) {
                return std::ops::ControlFlow::Continue(());
            }
            if r.contained_by(with_token_start(&c, source_file)) {
                candidate = Some(c);
                return std::ops::ControlFlow::Break(());
            }
            std::ops::ControlFlow::Continue(())
        });
        if let Some(candidate) = candidate {
            return find(&candidate, r, source_file);
        }

        n.clone()
    }
    let source_file_node = source_file.as_node();
    find(&source_file_node, r, source_file)
}

/**
 * Start of the original range might fall inside the comment - scanner will not yield appropriate results
 * This function will look for token that is located before the start of target range
 * and return its end as start position for the scanner.
 */
pub fn get_scan_start_position(
    enclosing_node: &ast::Node,
    original_range: core::TextRange,
    source_file: &ast::SourceFile,
) -> i32 {
    let adjusted = with_token_start(enclosing_node, source_file);
    let start = adjusted.pos();
    let enclosing_loc = source_file.store().loc(*enclosing_node);
    if start == original_range.pos() && enclosing_loc.end() == original_range.end() {
        return start;
    }

    let preceding_token = astnav::find_preceding_token(source_file, original_range.pos());
    let Some(preceding_token) = preceding_token else {
        // no preceding token found - start from the beginning of enclosing node
        return enclosing_loc.pos();
    };

    // preceding token ends after the start of original range (i.e when originalRange.pos falls in the middle of literal)
    // start from the beginning of enclosingNode to handle the entire 'originalRange'
    if source_file.store().loc(preceding_token).end() >= original_range.pos() {
        return enclosing_loc.pos();
    }

    source_file.store().loc(preceding_token).end()
}

/*
 * For cases like
 * if (a ||
 *     b ||$
 *     c) {...}
 * If we hit Enter at $ we want line '    b ||' to be indented.
 * Formatting will be applied to the last two lines.
 * Node that fully encloses these lines is binary expression 'a ||...'.
 * Initial indentation for this node will be 0.
 * Binary expressions don't introduce new indentation scopes, however it is possible
 * that some parent node on the same line does - like if statement in this case.
 * Note that we are considering parents only from the same line with initial node -
 * if parent is on the different line - its delta was already contributed
 * to the initial indentation.
 */
pub fn get_own_or_inherited_delta(
    mut n: Option<ast::Node>,
    options: lsutil::FormatCodeSettings,
    source_file: &ast::SourceFile,
) -> i32 {
    let store = source_file.store();
    let mut previous_line: i32 = -1;
    let mut child: Option<ast::Node> = None;
    while let Some(node) = n {
        let line = scanner::get_ecma_line_of_position(
            source_file,
            with_token_start(&node, source_file).pos(),
        );
        if previous_line != -1 && line as i32 != previous_line {
            break;
        }

        if should_indent_child_node(options.clone(), &node, child.as_ref(), source_file, false) {
            return options.editor_settings.indent_size;
        }

        previous_line = line as i32;
        child = Some(node.clone());
        n = store.parent(node);
    }
    0
}

pub fn range_has_no_errors(_r: core::TextRange) -> bool {
    false
}

pub fn prepare_range_contains_error_function(
    errors: Vec<ast::Diagnostic>,
    original_range: core::TextRange,
) -> Box<dyn FnMut(core::TextRange) -> bool> {
    if errors.is_empty() {
        return Box::new(range_has_no_errors);
    }

    // pick only errors that fall in range
    let mut sorted: Vec<_> = errors
        .into_iter()
        .filter(|d| original_range.overlaps(d.loc()))
        .collect();
    if sorted.is_empty() {
        return Box::new(range_has_no_errors);
    }
    sorted.sort_by_key(|d| d.pos());

    let mut index = 0usize;
    Box::new(move |r| {
        // in current implementation sequence of arguments [r1, r2...] is monotonically increasing.
        // 'index' tracks the index of the most recent error that was checked.
        loop {
            if index >= sorted.len() {
                // all errors in the range were already checked -> no error in specified range
                return false;
            }

            let err = &sorted[index];

            if r.end() <= err.pos() {
                // specified range ends before the error referred by 'index' - no error in range
                return false;
            }

            if r.overlaps(err.loc()) {
                // specified range overlaps with error range
                return true;
            }

            index += 1;
        }
    })
}

pub struct FormatSpanWorker {
    pub original_range: core::TextRange,
    pub enclosing_node: ast::Node,
    pub initial_indentation: i32,
    pub delta: i32,
    pub request_kind: FormatRequestKind,
    pub range_contains_error: Box<dyn FnMut(core::TextRange) -> bool>,
    pub source_file: ast::SourceFile,

    pub ctx: FormatContext,

    pub formatting_scanner: Option<FormattingScanner>,
    pub formatting_context: Option<FormattingContext>,

    pub edits: Vec<core::TextChange>,
    pub previous_range: TextRangeWithKind,
    pub previous_range_trivia_end: i32,
    pub previous_parent: Option<ast::Node>,
    pub previous_range_start_line: i32,

    pub child_context_node: Option<ast::Node>,
    pub last_indented_line: i32,
    pub indentation_on_last_indented_line: i32,

    pub visiting_node: Option<ast::Node>,
    pub visiting_indenter: Option<DynamicIndenter>,
    pub visiting_node_start_line: i32,
    pub visiting_undecorated_node_start_line: i32,

    pub current_rules: Vec<crate::RuleImpl>,
}

pub fn new_format_span_worker(
    ctx: FormatContext,
    original_range: core::TextRange,
    enclosing_node: ast::Node,
    initial_indentation: i32,
    delta: i32,
    request_kind: FormatRequestKind,
    range_contains_error: Box<dyn FnMut(core::TextRange) -> bool>,
    source_file: ast::SourceFile,
) -> FormatSpanWorker {
    FormatSpanWorker {
        ctx,
        original_range,
        enclosing_node,
        initial_indentation,
        delta,
        request_kind,
        range_contains_error,
        source_file,
        formatting_scanner: None,
        formatting_context: None,
        edits: Vec::new(),
        previous_range: new_text_range_with_kind(0, 0, ast::Kind::Unknown),
        previous_range_trivia_end: 0,
        previous_parent: None,
        previous_range_start_line: 0,
        child_context_node: None,
        last_indented_line: -1,
        indentation_on_last_indented_line: -1,
        visiting_node: None,
        visiting_indenter: None,
        visiting_node_start_line: 0,
        visiting_undecorated_node_start_line: 0,
        current_rules: Vec::with_capacity(32), // increaseInsertionIndex should assert there are no more than 32 rules in a given bucket
    }
}

pub fn get_non_decorator_token_pos_of_node(node: &ast::Node, file: &ast::SourceFile) -> i32 {
    let mut last_decorator = None;
    if ast::has_decorators(file.store(), *node) {
        last_decorator = file
            .store()
            .modifier_nodes(*node)
            .iter()
            .rev()
            .find(|n| ast::is_decorator(file.store(), **n))
            .cloned();
    }
    if last_decorator.is_none() {
        return with_token_start(node, file).pos();
    }
    scanner::skip_trivia(
        file.text(),
        file.store().loc(last_decorator.unwrap()).end() as usize,
    ) as i32
}

impl FormatSpanWorker {
    pub fn execute(&mut self, s: FormattingScanner) -> Vec<core::TextChange> {
        self.formatting_scanner = Some(s);
        self.indentation_on_last_indented_line = -1;
        self.last_indented_line = -1;
        let opt = get_format_code_settings_from_context(&self.ctx);
        self.formatting_context = Some(new_formatting_context(
            self.source_file.share_readonly(),
            self.request_kind,
            opt.clone(),
        ));
        // formatting context is used by rules provider

        self.scanner_mut().advance();

        if self.scanner().is_on_token() {
            let start_line = scanner::get_ecma_line_of_position(
                &self.source_file,
                with_token_start(&self.enclosing_node, &self.source_file).pos(),
            ) as i32;
            let mut undecorated_start_line = start_line;
            if ast::has_decorators(self.source_file.store(), self.enclosing_node) {
                undecorated_start_line = scanner::get_ecma_line_of_position(
                    &self.source_file,
                    get_non_decorator_token_pos_of_node(&self.enclosing_node, &self.source_file),
                ) as i32;
            }

            self.process_node(
                self.enclosing_node.clone(),
                self.enclosing_node.clone(),
                start_line,
                undecorated_start_line,
                self.initial_indentation,
                self.delta,
            );
        }

        // Leading trivia items get attached to and processed with the token that proceeds them. If the
        // range ends in the middle of some leading trivia, the token that proceeds them won't be in the
        // range and thus won't get processed. So we process those remaining trivia items here.
        let remaining_trivia = self.scanner().get_current_leading_trivia();
        if !remaining_trivia.is_empty() {
            let mut indentation = self.initial_indentation;
            if node_will_indent_child(
                self.formatting_context().options.clone(),
                &self.enclosing_node,
                None,
                Some(&self.source_file),
                false,
            ) {
                indentation += opt.editor_settings.indent_size;
            }

            self.indent_trivia_items(
                remaining_trivia.clone(),
                indentation,
                true,
                |worker, item| {
                    let (start_line, start_char) =
                        scanner::get_ecma_line_and_byte_offset_of_position(
                            &worker.source_file,
                            item.loc.pos(),
                        );
                    worker.process_range(
                        item.clone(),
                        start_line as i32,
                        start_char as i32,
                        worker.enclosing_node.clone(),
                        worker.enclosing_node.clone(),
                        None,
                    );
                    worker.insert_indentation(item.loc.pos(), indentation, false);
                },
            );

            if opt.editor_settings.trim_trailing_whitespace.is_true() {
                self.trim_trailing_whitespaces_for_remaining_range(remaining_trivia);
            }
        }

        if self.previous_range != new_text_range_with_kind(0, 0, ast::Kind::Unknown)
            && self.scanner().get_token_full_start() >= self.original_range.end()
        {
            // Formatting edits happen by looking at pairs of contiguous tokens (see `processPair`),
            // typically inserting or deleting whitespace between them. The recursive `processNode`
            // logic above bails out as soon as it encounters a token that is beyond the end of the
            // range we're supposed to format (or if we reach the end of the file). But this potentially
            // leaves out an edit that would occur *inside* the requested range but cannot be discovered
            // without looking at one token *beyond* the end of the range: consider the line `x = { }`
            // with a selection from the beginning of the line to the space inside the curly braces,
            // inclusive. We would expect a format-selection would delete the space (if rules apply),
            // but in order to do that, we need to process the pair ["{", "}"], but we stopped processing
            // just before getting there. This block handles this trailing edit.
            let mut token_info = None;
            if self.scanner().is_on_eof() {
                token_info = Some(self.scanner().read_eof_token_range());
            } else if self.scanner().is_on_token() {
                let enclosing_node = self.enclosing_node.clone();
                token_info = Some(self.read_token_info(&enclosing_node).token);
            }

            if let Some(token_info) = token_info {
                if token_info.loc.pos() == self.previous_range_trivia_end {
                    // We need to check that tokenInfo and previousRange are contiguous: the `originalRange`
                    // may have ended in the middle of a token, which means we will have stopped formatting
                    // on that token, leaving `previousRange` pointing to the token before it, but already
                    // having moved the formatting scanner (where we just got `tokenInfo`) to the next token.
                    // If this happens, our supposed pair [previousRange, tokenInfo] actually straddles the
                    // token that intersects the end of the range we're supposed to format, so the pair will
                    // produce bogus edits if we try to `processPair`. Recall that the point of this logic is
                    // to perform a trailing edit at the end of the selection range: but there can be no valid
                    // edit in the middle of a token where the range ended, so if we have a non-contiguous
                    // pair here, we're already done and we can ignore it.
                    let mut parent =
                        astnav::find_preceding_token(&self.source_file, token_info.loc.end())
                            .and_then(|token| self.source_file.store().parent(token));
                    if parent.is_none() {
                        parent = self.previous_parent.clone();
                    }
                    let line =
                        scanner::get_ecma_line_of_position(&self.source_file, token_info.loc.pos())
                            as i32;
                    self.process_pair(
                        token_info,
                        line,
                        parent.clone(),
                        self.previous_range.clone(),
                        self.previous_range_start_line,
                        self.previous_parent.clone(),
                        parent,
                        None,
                    );
                }
            }
        }

        self.edits.clone()
    }

    pub fn process_child_node(
        &mut self,
        node: ast::Node,
        _indenter: Option<DynamicIndenter>,
        _node_start_line: i32,
        _undecorated_node_start_line: i32,
        child: ast::Node,
        mut inherited_indentation: i32,
        parent: ast::Node,
        parent_dynamic_indentation: Option<DynamicIndenter>,
        parent_start_line: i32,
        undecorated_parent_start_line: i32,
        is_list_item: bool,
        is_first_list_item: bool,
    ) -> i32 {
        let store = self.source_file.store();
        let child_loc = store.loc(child);
        let parent_loc = store.loc(parent);
        let child_kind = store.kind(child);
        debug_assert!(!ast::node_is_synthesized(store, child));

        if ast::node_is_missing(store, Some(child))
            || is_grammar_error(self.source_file.store(), &parent, &child)
            || store.flags(child).intersects(ast::NodeFlags::REPARSED)
        {
            return inherited_indentation;
        }

        let child_start_pos =
            scanner::get_token_pos_of_node(&child, &self.source_file, false) as i32;
        let child_start_line =
            scanner::get_ecma_line_of_position(&self.source_file, child_start_pos) as i32;

        let mut undecorated_child_start_line = child_start_line;
        if ast::has_decorators(self.source_file.store(), child) {
            undecorated_child_start_line = scanner::get_ecma_line_of_position(
                &self.source_file,
                get_non_decorator_token_pos_of_node(&child, &self.source_file),
            ) as i32;
        }

        // if child is a list item - try to get its indentation, only if parent is within the original range.
        let mut child_indentation_amount = -1;

        if is_list_item && parent_loc.contained_by(self.original_range) {
            child_indentation_amount = self.try_compute_indentation_for_list_item(
                child_start_pos,
                child_loc.end(),
                parent_start_line,
                self.original_range,
                inherited_indentation,
            );
            if child_indentation_amount != -1 {
                inherited_indentation = child_indentation_amount;
            }
        }

        // child node is outside the target range - do not dive inside
        if !self.original_range.overlaps(child_loc) {
            if child_loc.end() < self.original_range.pos() {
                self.scanner_mut().skip_to_end_of(&child_loc);
            }
            return inherited_indentation;
        }

        if child_loc.len() == 0 {
            return inherited_indentation;
        }

        while self.scanner().is_on_token()
            && self.scanner().get_token_full_start() < self.original_range.end()
        {
            // proceed any parent tokens that are located prior to child.getStart()
            let token_info = self.read_token_info(&node);
            if token_info.token.loc.end() > self.original_range.end() {
                return inherited_indentation;
            }
            if token_info.token.loc.end() > child_start_pos {
                if token_info.token.loc.pos() > child_start_pos {
                    self.scanner_mut().skip_to_start_of(&child_loc);
                }
                // stop when formatting scanner advances past the beginning of the child
                break;
            }

            self.consume_token_and_advance_scanner(
                token_info,
                node.clone(),
                parent_dynamic_indentation.clone(),
                node.clone(),
                false,
            );
        }

        if !self.scanner().is_on_token()
            || self.scanner().get_token_full_start() >= self.original_range.end()
        {
            return inherited_indentation;
        }

        if ast::is_token_kind(child_kind) {
            // if child node is a token, it does not impact indentation, proceed it using parent indentation scope rules
            let token_info = self.read_token_info(&child);
            // JSX text shouldn't affect indenting
            if child_kind != ast::Kind::JsxText {
                debug_assert!(
                    token_info.token.loc.end() == child_loc.end(),
                    "Token end is child end"
                );
                self.consume_token_and_advance_scanner(
                    token_info,
                    node,
                    parent_dynamic_indentation,
                    child,
                    false,
                );
                return inherited_indentation;
            }
        }

        let mut effective_parent_start_line = undecorated_parent_start_line;
        if child_kind == ast::Kind::Decorator {
            effective_parent_start_line = child_start_line;
        }
        let (child_indentation, delta) = self.compute_indentation(
            child.clone(),
            child_start_line,
            child_indentation_amount,
            parent.clone(),
            parent_dynamic_indentation.clone(),
            effective_parent_start_line,
        );
        let context = self
            .child_context_node
            .clone()
            .unwrap_or_else(|| parent.clone());
        self.process_node(
            child.clone(),
            context,
            child_start_line,
            undecorated_child_start_line,
            child_indentation,
            delta,
        );

        self.child_context_node = Some(parent.clone());

        if is_first_list_item
            && self.source_file.store().kind(parent) == ast::Kind::ArrayLiteralExpression
            && inherited_indentation == -1
        {
            inherited_indentation = child_indentation;
        }

        inherited_indentation
    }

    pub fn process_child_nodes(
        &mut self,
        node: ast::Node,
        indenter: Option<DynamicIndenter>,
        node_start_line: i32,
        undecorated_node_start_line: i32,
        nodes: ast::SourceNodeListInput,
        parent: ast::Node,
        parent_start_line: i32,
        parent_dynamic_indentation: Option<DynamicIndenter>,
    ) {
        debug_assert!(!ast::position_is_synthesized(nodes.loc().pos()));
        debug_assert!(!ast::position_is_synthesized(nodes.loc().end()));

        let list_start_token =
            get_open_token_for_list_input(self.source_file.store(), &parent, &nodes);

        let mut list_dynamic_indentation = parent_dynamic_indentation.clone();
        let mut start_line = parent_start_line;

        // node range is outside the target range - do not dive inside
        if !self.original_range.overlaps(nodes.loc()) {
            if nodes.loc().end() < self.original_range.pos()
                && (nodes.is_empty()
                    || !self
                        .source_file
                        .store()
                        .flags(nodes.iter().next().unwrap())
                        .intersects(ast::NodeFlags::REPARSED))
            {
                self.scanner_mut().skip_to_end_of(&nodes.loc());
            }
            return;
        }

        if list_start_token != ast::Kind::Unknown {
            // introduce a new indentation scope for lists (including list start and end tokens)
            while self.scanner().is_on_token()
                && self.scanner().get_token_full_start() < self.original_range.end()
            {
                let token_info = self.read_token_info(&parent);
                if token_info.token.loc.end() > nodes.loc().pos() {
                    // stop when formatting scanner moves past the beginning of node list
                    break;
                } else if token_info.token.kind == list_start_token {
                    // consume list start token
                    start_line = scanner::get_ecma_line_of_position(
                        &self.source_file,
                        token_info.token.loc.pos(),
                    ) as i32;

                    self.consume_token_and_advance_scanner(
                        token_info.clone(),
                        parent.clone(),
                        parent_dynamic_indentation.clone(),
                        parent.clone(),
                        false,
                    );

                    let indentation_on_list_start_token =
                        if self.indentation_on_last_indented_line != -1 {
                            // scanner just processed list start token so consider last indentation as list indentation
                            // function foo(): { // last indentation was 0, list item will be indented based on this value
                            //   foo: number;
                            // }: {};
                            self.indentation_on_last_indented_line
                        } else {
                            let start_line_position = get_line_start_position_for_position(
                                token_info.token.loc.pos(),
                                &self.source_file,
                            );
                            find_first_non_whitespace_column(
                                start_line_position,
                                token_info.token.loc.pos(),
                                &self.source_file,
                                self.formatting_context().options.clone(),
                            )
                        };

                    list_dynamic_indentation = Some(
                        self.get_dynamic_indentation(
                            parent.clone(),
                            parent_start_line,
                            indentation_on_list_start_token,
                            self.formatting_context()
                                .options
                                .editor_settings
                                .indent_size,
                        ),
                    );
                } else {
                    // consume any tokens that precede the list as child elements of 'node' using its indentation scope
                    self.consume_token_and_advance_scanner(
                        token_info,
                        parent.clone(),
                        parent_dynamic_indentation.clone(),
                        parent.clone(),
                        false,
                    );
                }
            }
        }

        let mut inherited_indentation = -1;
        for (i, child) in nodes.iter().enumerate() {
            inherited_indentation = self.process_child_node(
                node.clone(),
                indenter.clone(),
                node_start_line,
                undecorated_node_start_line,
                child,
                inherited_indentation,
                node.clone(),
                list_dynamic_indentation.clone(),
                start_line,
                start_line,
                true,
                i == 0,
            );
        }

        let list_end_token = get_close_token_for_open_token(list_start_token);
        if list_end_token != ast::Kind::Unknown
            && self.scanner().is_on_token()
            && self.scanner().get_token_full_start() < self.original_range.end()
        {
            let mut token_info = self.read_token_info(&parent);
            if token_info.token.kind == ast::Kind::CommaToken {
                // consume the comma
                self.consume_token_and_advance_scanner(
                    token_info,
                    parent.clone(),
                    list_dynamic_indentation.clone(),
                    parent.clone(),
                    false,
                );
                if self.scanner().is_on_token() {
                    token_info = self.read_token_info(&parent);
                } else {
                    return;
                }
            }

            // consume the list end token only if it is still belong to the parent
            // there might be the case when current token matches end token but does not considered as one
            // function (x: function) <--
            // without this check close paren will be interpreted as list end token for function expression which is wrong
            if token_info.token.kind == list_end_token
                && token_info
                    .token
                    .loc
                    .contained_by(self.source_file.store().loc(parent))
            {
                // consume list end token
                self.consume_token_and_advance_scanner(
                    token_info,
                    parent.clone(),
                    list_dynamic_indentation,
                    parent,
                    true,
                );
            }
        }
    }

    pub fn execute_process_node_visitor(
        &mut self,
        node: ast::Node,
        indenter: DynamicIndenter,
        node_start_line: i32,
        undecorated_node_start_line: i32,
    ) {
        let old_node = self.visiting_node.clone();
        let old_indenter = self.visiting_indenter.clone();
        let old_start = self.visiting_node_start_line;
        let old_undecorated_start = self.visiting_undecorated_node_start_line;
        self.visiting_node = Some(node.clone());
        self.visiting_indenter = Some(indenter);
        self.visiting_node_start_line = node_start_line;
        self.visiting_undecorated_node_start_line = undecorated_node_start_line;
        let children = {
            let children = RefCell::new(Vec::new());
            let _ = self.source_file.store().visit_each_child_with_lists(
                node,
                |child| {
                    if let Some(child) = child {
                        children.borrow_mut().push(VisitEachChildItem::Node(child));
                    }
                    ControlFlow::Continue(())
                },
                |nodes| {
                    children.borrow_mut().push(VisitEachChildItem::NodeList(
                        ast::SourceNodeListInput::from_source(nodes),
                    ));
                    ControlFlow::Continue(())
                },
                |modifiers| {
                    children.borrow_mut().push(VisitEachChildItem::ModifierList(
                        ast::SourceModifierListInput::from_source(modifiers),
                    ));
                    ControlFlow::Continue(())
                },
            );
            children.into_inner()
        };
        for child in children {
            let Some(visiting_node) = self.visiting_node else {
                continue;
            };
            let visiting_indenter = self.visiting_indenter.clone();
            let visiting_node_start_line = self.visiting_node_start_line;
            let visiting_undecorated_node_start_line = self.visiting_undecorated_node_start_line;
            match child {
                VisitEachChildItem::Node(child) => {
                    self.process_child_node(
                        visiting_node,
                        visiting_indenter.clone(),
                        visiting_node_start_line,
                        visiting_undecorated_node_start_line,
                        child,
                        -1,
                        visiting_node,
                        visiting_indenter,
                        visiting_node_start_line,
                        visiting_undecorated_node_start_line,
                        false,
                        false,
                    );
                }
                VisitEachChildItem::NodeList(nodes) => {
                    self.process_child_nodes(
                        visiting_node,
                        visiting_indenter.clone(),
                        visiting_node_start_line,
                        visiting_undecorated_node_start_line,
                        nodes,
                        visiting_node,
                        visiting_node_start_line,
                        visiting_indenter,
                    );
                }
                VisitEachChildItem::ModifierList(modifiers) => {
                    for child in modifiers.iter() {
                        self.process_child_node(
                            visiting_node,
                            visiting_indenter.clone(),
                            visiting_node_start_line,
                            visiting_undecorated_node_start_line,
                            child,
                            -1,
                            visiting_node,
                            visiting_indenter.clone(),
                            visiting_node_start_line,
                            visiting_undecorated_node_start_line,
                            false,
                            false,
                        );
                    }
                }
            }
        }
        self.visiting_node = old_node;
        self.visiting_indenter = old_indenter;
        self.visiting_node_start_line = old_start;
        self.visiting_undecorated_node_start_line = old_undecorated_start;
    }

    pub fn compute_indentation(
        &self,
        node: ast::Node,
        start_line: i32,
        inherited_indentation: i32,
        parent: ast::Node,
        parent_dynamic_indentation: Option<DynamicIndenter>,
        effective_parent_start_line: i32,
    ) -> (i32, i32) {
        let parent_dynamic_indentation = parent_dynamic_indentation.unwrap_or_else(|| {
            self.get_dynamic_indentation(
                parent.clone(),
                effective_parent_start_line,
                self.initial_indentation,
                self.delta,
            )
        });
        let mut delta = 0;
        if should_indent_child_node(
            self.formatting_context().options.clone(),
            &node,
            None,
            &self.formatting_context().source_file,
            false,
        ) {
            delta = self
                .formatting_context()
                .options
                .editor_settings
                .indent_size;
        }

        if effective_parent_start_line == start_line {
            // if node is located on the same line with the parent
            // - inherit indentation from the parent
            // - push children if either parent of node itself has non-zero delta
            let mut indentation = self.indentation_on_last_indented_line;
            if start_line != self.last_indented_line {
                indentation = parent_dynamic_indentation.get_indentation();
            }
            delta = cmp::min(
                self.formatting_context()
                    .options
                    .editor_settings
                    .indent_size,
                parent_dynamic_indentation.get_delta(&node) + delta,
            );
            return (indentation, delta);
        } else if inherited_indentation == -1 {
            if self.source_file.store().kind(node) == ast::Kind::OpenParenToken
                && start_line == self.last_indented_line
            {
                // the is used for chaining methods formatting
                // - we need to get the indentation on last line and the delta of parent
                return (
                    self.indentation_on_last_indented_line,
                    parent_dynamic_indentation.get_delta(&node),
                );
            } else if child_starts_on_the_same_line_with_else_in_if_statement(
                &parent,
                &node,
                start_line,
                &self.source_file,
            ) || child_is_unindented_branch_of_conditional_expression(
                &parent,
                &node,
                start_line,
                &self.source_file,
            ) || argument_starts_on_same_line_as_previous_argument(
                &parent,
                &node,
                start_line,
                &self.source_file,
            ) {
                return (parent_dynamic_indentation.get_indentation(), delta);
            } else {
                let i = parent_dynamic_indentation.get_indentation();
                if i == -1 {
                    return (parent_dynamic_indentation.get_indentation(), delta);
                }
                return (i + parent_dynamic_indentation.get_delta(&node), delta);
            }
        }

        (inherited_indentation, delta)
    }

    /** Tries to compute the indentation for a list element.
     * If list element is not in range then
     * function will pick its actual indentation
     * so it can be pushed downstream as inherited indentation.
     * If list element is in the range - its indentation will be equal
     * to inherited indentation from its predecessors.
     */
    pub fn try_compute_indentation_for_list_item(
        &self,
        start_pos: i32,
        end_pos: i32,
        parent_start_line: i32,
        r: core::TextRange,
        inherited_indentation: i32,
    ) -> i32 {
        let r2 = core::new_text_range(start_pos, end_pos);
        if r.overlaps(r2) || r2.contained_by(r) {
            /* Not to miss zero-range nodes e.g. JsxText */
            if inherited_indentation != -1 {
                return inherited_indentation;
            }
        } else {
            let start_line = scanner::get_ecma_line_of_position(&self.source_file, start_pos);
            let start_line_position =
                get_line_start_position_for_position(start_pos, &self.source_file);
            let column = find_first_non_whitespace_column(
                start_line_position,
                start_pos,
                &self.source_file,
                self.formatting_context().options.clone(),
            );
            if start_line as i32 != parent_start_line || start_pos == column {
                // Use the base indent size if it is greater than
                // the indentation of the inherited predecessor.
                let base_indent_size = self
                    .formatting_context()
                    .options
                    .editor_settings
                    .base_indent_size;
                if base_indent_size > column {
                    return base_indent_size;
                }
                return column;
            }
        }
        -1
    }

    pub fn process_node(
        &mut self,
        node: ast::Node,
        context_node: ast::Node,
        node_start_line: i32,
        undecorated_node_start_line: i32,
        indentation: i32,
        delta: i32,
    ) {
        if !self
            .original_range
            .overlaps(with_token_start(&node, &self.source_file))
        {
            return;
        }

        let node_dynamic_indentation =
            self.get_dynamic_indentation(node.clone(), node_start_line, indentation, delta);

        // a useful observations when tracking context node
        //        /
        //      [a]
        //   /   |   \
        //  [b] [c] [d]
        // node 'a' is a context node for nodes 'b', 'c', 'd'
        // except for the leftmost leaf token in [b] - in this case context node ('e') is located somewhere above 'a'
        // this rule can be applied recursively to child nodes of 'a'.
        //
        // context node is set to parent node value after processing every child node
        // context node is set to parent of the token after processing every token

        self.child_context_node = Some(context_node);

        // if there are any tokens that logically belong to node and interleave child nodes
        // such tokens will be consumed in processChildNode for the child that follows them
        self.execute_process_node_visitor(
            node.clone(),
            node_dynamic_indentation.clone(),
            node_start_line,
            undecorated_node_start_line,
        );

        // proceed any tokens in the node that are located after child nodes
        while self.scanner().is_on_token()
            && self.scanner().get_token_full_start() < self.original_range.end()
        {
            let token_info = self.read_token_info(&node);
            if token_info.token.loc.end()
                > cmp::min(
                    self.source_file.store().loc(node).end(),
                    self.original_range.end(),
                )
            {
                break;
            }
            self.consume_token_and_advance_scanner(
                token_info,
                node.clone(),
                Some(node_dynamic_indentation.clone()),
                node.clone(),
                false,
            );
        }
    }

    pub fn process_pair(
        &mut self,
        current_item: TextRangeWithKind,
        current_start_line: i32,
        current_parent: Option<ast::Node>,
        previous_item: TextRangeWithKind,
        previous_start_line: i32,
        previous_parent: Option<ast::Node>,
        context_node: Option<ast::Node>,
        mut dynamic_indentation: Option<DynamicIndenter>,
    ) -> LineAction {
        self.formatting_context_mut().update_context(
            previous_item.clone(),
            previous_parent.clone(),
            current_item.clone(),
            current_parent.clone(),
            context_node.clone(),
        );

        self.current_rules.clear();
        self.current_rules = get_rules(self.formatting_context_mut(), Vec::new());

        let mut trim_trailing_whitespaces = !self
            .formatting_context()
            .options
            .editor_settings
            .trim_trailing_whitespace
            .is_false();
        let mut line_action = LINE_ACTION_NONE;

        if !self.current_rules.is_empty() {
            // Apply rules in reverse order so that higher priority rules (which are first in the array)
            // win in a conflict with lower priority rules.
            for rule in self.current_rules.clone().into_iter().rev() {
                line_action = self.apply_rule_edits(
                    &rule,
                    previous_item.clone(),
                    previous_start_line,
                    current_item.clone(),
                    current_start_line,
                );
                if let Some(indentation) = dynamic_indentation.as_mut() {
                    match line_action {
                        LINE_ACTION_LINE_REMOVED => {
                            // Handle the case where the next line is moved to be the end of this line.
                            // In this case we don't indent the next line in the next pass.
                            if current_parent.as_ref().is_some_and(|parent| {
                                scanner::get_token_pos_of_node(parent, &self.source_file, false)
                                    as i32
                                    == current_item.loc.pos()
                            }) {
                                indentation.recompute_indentation(
                                    false, /*lineAddedByFormatting*/
                                    context_node.as_ref(),
                                );
                            }
                        }
                        LINE_ACTION_LINE_ADDED => {
                            // Handle the case where token2 is moved to the new line.
                            // In this case we indent token2 in the next pass but we set
                            // sameLineIndent flag to notify the indenter that the indentation is within the line.
                            if current_parent.as_ref().is_some_and(|parent| {
                                scanner::get_token_pos_of_node(parent, &self.source_file, false)
                                    as i32
                                    == current_item.loc.pos()
                            }) {
                                indentation.recompute_indentation(
                                    true, /*lineAddedByFormatting*/
                                    context_node.as_ref(),
                                );
                            }
                        }
                        _ => debug_assert!(line_action == LINE_ACTION_NONE),
                    }
                }

                // We need to trim trailing whitespace between the tokens if they were on different lines, and no rule was applied to put them on the same line
                trim_trailing_whitespaces = trim_trailing_whitespaces
                    && (rule.action().0 & RuleAction::DELETE_SPACE.0 == 0)
                    && rule.flags() != RuleFlags::CAN_DELETE_NEW_LINES;
            }
        } else {
            trim_trailing_whitespaces =
                trim_trailing_whitespaces && current_item.kind != ast::Kind::EndOfFile;
        }

        if current_start_line != previous_start_line && trim_trailing_whitespaces {
            // We need to trim trailing whitespace between the tokens if they were on different lines, and no rule was applied to put them on the same line
            self.trim_trailing_whitespaces_for_lines(
                previous_start_line,
                current_start_line,
                previous_item,
            );
        }

        line_action
    }

    pub fn apply_rule_edits(
        &mut self,
        rule: &crate::RuleImpl,
        previous_range: TextRangeWithKind,
        previous_start_line: i32,
        current_range: TextRangeWithKind,
        current_start_line: i32,
    ) -> LineAction {
        let on_later_line = current_start_line != previous_start_line;
        match rule.action() {
            action if action == RuleAction::STOP_PROCESSING_SPACE_ACTIONS => {
                // no action required
                return LINE_ACTION_NONE;
            }
            action if action == RuleAction::DELETE_SPACE => {
                if previous_range.loc.end() != current_range.loc.pos() {
                    // delete characters starting from t1.end up to t2.pos exclusive
                    self.record_delete(
                        previous_range.loc.end(),
                        current_range.loc.pos() - previous_range.loc.end(),
                    );
                    if on_later_line {
                        return LINE_ACTION_LINE_REMOVED;
                    }
                    return LINE_ACTION_NONE;
                }
            }
            action if action == RuleAction::DELETE_TOKEN => {
                self.record_delete(previous_range.loc.pos(), previous_range.loc.len());
            }
            action if action == RuleAction::INSERT_NEW_LINE => {
                // exit early if we on different lines and rule cannot change number of newlines
                // if line1 and line2 are on subsequent lines then no edits are required - ok to exit
                // if line1 and line2 are separated with more than one newline - ok to exit since we cannot delete extra new lines
                if rule.flags() != RuleFlags::CAN_DELETE_NEW_LINES
                    && previous_start_line != current_start_line
                {
                    return LINE_ACTION_NONE;
                }

                // edit should not be applied if we have one line feed between elements
                let line_delta = current_start_line - previous_start_line;
                if line_delta != 1 {
                    self.record_replace(
                        previous_range.loc.end(),
                        current_range.loc.pos() - previous_range.loc.end(),
                        get_new_line_or_default_from_context(&self.ctx),
                    );
                    if on_later_line {
                        return LINE_ACTION_NONE;
                    }
                    return LINE_ACTION_LINE_ADDED;
                }
            }
            action if action == RuleAction::INSERT_SPACE => {
                // exit early if we on different lines and rule cannot change number of newlines
                if rule.flags() != RuleFlags::CAN_DELETE_NEW_LINES
                    && previous_start_line != current_start_line
                {
                    return LINE_ACTION_NONE;
                }

                let pos_delta = current_range.loc.pos() - previous_range.loc.end();
                if pos_delta != 1
                    || !self.source_file.text()[previous_range.loc.end() as usize..]
                        .starts_with(' ')
                {
                    self.record_replace(previous_range.loc.end(), pos_delta, " ".to_string());
                    if on_later_line {
                        return LINE_ACTION_LINE_REMOVED;
                    }
                    return LINE_ACTION_NONE;
                }
            }
            action if action == RuleAction::INSERT_TRAILING_SEMICOLON => {
                self.record_insert(previous_range.loc.end(), ";".to_string());
            }
            _ => {}
        }
        LINE_ACTION_NONE
    }

    pub fn process_range(
        &mut self,
        r: TextRangeWithKind,
        range_start_line: i32,
        _range_start_character: i32,
        parent: ast::Node,
        context_node: ast::Node,
        dynamic_indentation: Option<DynamicIndenter>,
    ) -> LineAction {
        let range_has_error = (self.range_contains_error)(r.loc);
        let mut line_action = LINE_ACTION_NONE;
        if !range_has_error {
            if self.previous_range == new_text_range_with_kind(0, 0, ast::Kind::Unknown) {
                // trim whitespaces starting from the beginning of the span up to the current line
                let original_start_line = scanner::get_ecma_line_of_position(
                    &self.source_file,
                    self.original_range.pos(),
                );
                self.trim_trailing_whitespaces_for_lines(
                    original_start_line as i32,
                    range_start_line,
                    new_text_range_with_kind(0, 0, ast::Kind::Unknown),
                );
            } else {
                line_action = self.process_pair(
                    r.clone(),
                    range_start_line,
                    Some(parent.clone()),
                    self.previous_range.clone(),
                    self.previous_range_start_line,
                    self.previous_parent.clone(),
                    Some(context_node),
                    dynamic_indentation,
                );
            }
        }

        self.previous_range = r.clone();
        self.previous_range_trivia_end = r.loc.end();
        self.previous_parent = Some(parent);
        self.previous_range_start_line = range_start_line;

        line_action
    }

    pub fn process_trivia(
        &mut self,
        trivia: Vec<TextRangeWithKind>,
        parent: ast::Node,
        context_node: Option<ast::Node>,
        dynamic_indentation: Option<DynamicIndenter>,
    ) {
        for trivia_item in trivia {
            if is_comment(trivia_item.kind) && trivia_item.loc.contained_by(self.original_range) {
                let (trivia_item_start_line, trivia_item_start_character) =
                    scanner::get_ecma_line_and_byte_offset_of_position(
                        &self.source_file,
                        trivia_item.loc.pos(),
                    );
                self.process_range(
                    trivia_item,
                    trivia_item_start_line as i32,
                    trivia_item_start_character as i32,
                    parent.clone(),
                    context_node.clone().unwrap_or_else(|| parent.clone()),
                    dynamic_indentation.clone(),
                );
            }
        }
    }

    /**
     * Trimming will be done for lines after the previous range.
     * Exclude comments as they had been previously processed.
     */
    pub fn trim_trailing_whitespaces_for_remaining_range(
        &mut self,
        trivias: Vec<TextRangeWithKind>,
    ) {
        let mut start_pos = self.original_range.pos();
        if self.previous_range != new_text_range_with_kind(0, 0, ast::Kind::Unknown) {
            start_pos = self.previous_range.loc.end();
        }

        for trivia in trivias {
            if is_comment(trivia.kind) {
                if start_pos < trivia.loc.pos() {
                    self.trim_trailing_witespaces_for_positions(
                        start_pos,
                        trivia.loc.pos() - 1,
                        self.previous_range.clone(),
                    );
                }

                start_pos = trivia.loc.end() + 1;
            }
        }

        if start_pos < self.original_range.end() {
            self.trim_trailing_witespaces_for_positions(
                start_pos,
                self.original_range.end(),
                self.previous_range.clone(),
            );
        }
    }

    pub fn trim_trailing_witespaces_for_positions(
        &mut self,
        start_pos: i32,
        end_pos: i32,
        previous_range: TextRangeWithKind,
    ) {
        let start_line = scanner::get_ecma_line_of_position(&self.source_file, start_pos);
        let end_line = scanner::get_ecma_line_of_position(&self.source_file, end_pos);

        self.trim_trailing_whitespaces_for_lines(
            start_line as i32,
            end_line as i32 + 1,
            previous_range,
        );
    }

    pub fn trim_trailing_whitespaces_for_lines(
        &mut self,
        line1: i32,
        line2: i32,
        r: TextRangeWithKind,
    ) {
        let line_starts = scanner::get_ecma_line_starts(&self.source_file);
        for line in line1..line2 {
            let line_start_position = line_starts[line as usize] as i32;
            let line_end_position =
                scanner::get_ecma_end_line_position(&self.source_file, line as usize) as i32;

            // do not trim whitespaces in comments or template expression
            if r != new_text_range_with_kind(0, 0, ast::Kind::Unknown)
                && (is_comment(r.kind)
                    || is_string_or_regular_expression_or_template_literal(r.kind))
                && r.loc.pos() <= line_end_position
                && r.loc.end() > line_end_position
            {
                continue;
            }

            let whitespace_start =
                self.get_trailing_whitespace_start_position(line_start_position, line_end_position);
            if whitespace_start != -1 {
                if whitespace_start != line_start_position {
                    let ch =
                        self.source_file.text().as_bytes()[(whitespace_start - 1) as usize] as char;
                    debug_assert!(!is_white_space_single_line(ch));
                }
                self.record_delete(whitespace_start, line_end_position + 1 - whitespace_start);
            }
        }
    }

    /**
     * @param start The position of the first character in range
     * @param end The position of the last character in range
     */
    pub fn get_trailing_whitespace_start_position(&self, start: i32, end: i32) -> i32 {
        let mut pos = end;
        let text = self.source_file.text();
        while pos >= start {
            let Some(ch) = text[pos as usize..].chars().next() else {
                pos -= 1; // multibyte character, rewind more
                continue;
            };
            if !is_white_space_single_line(ch) {
                break;
            }
            pos -= 1;
        }
        if pos != end {
            return pos + 1;
        }
        -1
    }

    pub fn insert_indentation(&mut self, pos: i32, indentation: i32, line_added: bool) {
        let indentation_string =
            get_indentation_string(indentation, self.formatting_context().options.clone());
        if line_added {
            // new line is added before the token by the formatting rules
            // insert indentation string at the very beginning of the token
            self.record_replace(pos, 0, indentation_string);
        } else {
            let (token_start_line, token_start_character) =
                scanner::get_ecma_line_and_byte_offset_of_position(&self.source_file, pos);
            let start_line_position =
                scanner::get_ecma_line_starts(&self.source_file)[token_start_line] as i32;
            if indentation
                != self.character_to_column(start_line_position, token_start_character as i32)
                || self.indentation_is_different(indentation_string.clone(), start_line_position)
            {
                self.record_replace(
                    start_line_position,
                    token_start_character as i32,
                    indentation_string,
                );
            }
        }
    }

    pub fn character_to_column(&self, start_line_position: i32, character_in_line: i32) -> i32 {
        let mut column = 0;
        for i in 0..character_in_line {
            if self.source_file.text().as_bytes()[(start_line_position + i) as usize] == b'\t' {
                if self.formatting_context().options.editor_settings.tab_size > 0 {
                    column += self.formatting_context().options.editor_settings.tab_size
                        - (column % self.formatting_context().options.editor_settings.tab_size);
                }
            } else {
                column += 1;
            }
        }
        column
    }

    pub fn indentation_is_different(
        &self,
        indentation_string: String,
        start_line_position: i32,
    ) -> bool {
        let text = self.source_file.text();
        let end = start_line_position as usize + indentation_string.len();
        if end > text.len() {
            return true;
        }
        indentation_string != text[start_line_position as usize..end]
    }

    pub fn indent_trivia_items(
        &mut self,
        trivia: Vec<TextRangeWithKind>,
        comment_indentation: i32,
        mut indent_next_token_or_trivia: bool,
        mut indent_single_line: impl FnMut(&mut Self, TextRangeWithKind),
    ) -> bool {
        for trivia_item in trivia {
            let trivia_in_range = trivia_item.loc.contained_by(self.original_range);
            match trivia_item.kind {
                ast::Kind::MultiLineCommentTrivia => {
                    if trivia_in_range {
                        self.indent_multiline_comment(
                            trivia_item.loc,
                            comment_indentation,
                            !indent_next_token_or_trivia,
                            true,
                        );
                    }
                    indent_next_token_or_trivia = false;
                }
                ast::Kind::SingleLineCommentTrivia => {
                    if indent_next_token_or_trivia && trivia_in_range {
                        indent_single_line(self, trivia_item);
                    }
                    indent_next_token_or_trivia = false;
                }
                ast::Kind::NewLineTrivia => {
                    indent_next_token_or_trivia = true;
                }
                _ => {}
            }
        }
        indent_next_token_or_trivia
    }

    pub fn indent_multiline_comment(
        &mut self,
        comment_range: core::TextRange,
        indentation: i32,
        first_line_is_indented: bool,
        indent_final_line: bool,
    ) {
        // split comment in lines
        let mut start_line =
            scanner::get_ecma_line_of_position(&self.source_file, comment_range.pos());
        let end_line = scanner::get_ecma_line_of_position(&self.source_file, comment_range.end());

        if start_line == end_line {
            if !first_line_is_indented {
                // treat as single line comment
                self.insert_indentation(comment_range.pos(), indentation, false);
            }
            return;
        }

        let mut parts = Vec::with_capacity(
            self.source_file.text()[comment_range.pos() as usize..comment_range.end() as usize]
                .matches('\n')
                .count(),
        );
        let mut start_pos = comment_range.pos();
        for line in start_line..end_line {
            let end_of_line = scanner::get_ecma_end_line_position(&self.source_file, line) as i32;
            parts.push(core::new_text_range(start_pos, end_of_line));
            start_pos = scanner::get_ecma_line_starts(&self.source_file)[line as usize + 1] as i32;
        }

        if indent_final_line {
            parts.push(core::new_text_range(start_pos, comment_range.end()));
        }

        if parts.is_empty() {
            return;
        }

        let start_line_pos =
            scanner::get_ecma_line_starts(&self.source_file)[start_line as usize] as i32;

        let (non_whitespace_in_first_part_character, non_whitespace_in_first_part_column) =
            find_first_non_whitespace_character_and_column(
                start_line_pos,
                parts[0].pos(),
                &self.source_file,
                self.formatting_context().options.clone(),
            );

        let mut start_index = 0usize;

        if first_line_is_indented {
            start_index = 1;
            start_line += 1;
        }

        // shift all parts on the delta size
        let delta = indentation - non_whitespace_in_first_part_column;
        for i in start_index..parts.len() {
            let start_line_pos =
                scanner::get_ecma_line_starts(&self.source_file)[start_line as usize] as i32;
            let mut non_whitespace_character = non_whitespace_in_first_part_character;
            let mut non_whitespace_column = non_whitespace_in_first_part_column;
            if i != 0 {
                let found = find_first_non_whitespace_character_and_column(
                    parts[i].pos(),
                    parts[i].end(),
                    &self.source_file,
                    self.formatting_context().options.clone(),
                );
                non_whitespace_character = found.0;
                non_whitespace_column = found.1;
            }
            let new_indentation = non_whitespace_column + delta;
            if new_indentation > 0 {
                let indentation_string = get_indentation_string(
                    new_indentation,
                    self.formatting_context().options.clone(),
                );
                self.record_replace(start_line_pos, non_whitespace_character, indentation_string);
            } else {
                self.record_delete(start_line_pos, non_whitespace_character);
            }

            start_line += 1;
        }
    }

    pub fn record_delete(&mut self, start: i32, length: i32) {
        if length != 0 {
            self.edits.push(create_text_change_from_start_length(
                start,
                length,
                "".to_string(),
            ));
        }
    }

    pub fn record_replace(&mut self, start: i32, length: i32, new_text: String) {
        if length != 0 || !new_text.is_empty() {
            self.edits.push(create_text_change_from_start_length(
                start, length, new_text,
            ));
        }
    }

    pub fn record_insert(&mut self, start: i32, text: String) {
        if !text.is_empty() {
            self.edits
                .push(create_text_change_from_start_length(start, 0, text));
        }
    }

    pub fn consume_token_and_advance_scanner(
        &mut self,
        current_token_info: TokenInfo,
        parent: ast::Node,
        dynamic_indenation: Option<DynamicIndenter>,
        container: ast::Node,
        is_list_end_token: bool,
    ) {
        // assert(currentTokenInfo.token.Loc.ContainedBy(parent.Loc)) // !!!
        let last_trivia_was_new_line = self.scanner().last_trailing_trivia_was_new_line();
        let mut indent_token = false;
        let dynamic_indenation = dynamic_indenation.unwrap_or_else(|| {
            self.get_dynamic_indentation(
                parent.clone(),
                self.previous_range_start_line,
                self.initial_indentation,
                self.delta,
            )
        });

        if !current_token_info.leading_trivia.is_empty() {
            self.process_trivia(
                current_token_info.leading_trivia.clone(),
                parent.clone(),
                self.child_context_node.clone(),
                Some(dynamic_indenation.clone()),
            );
        }

        let mut line_action = LINE_ACTION_NONE;
        let is_token_in_range = current_token_info
            .token
            .loc
            .contained_by(self.original_range);

        let (token_start_line, token_start_char) =
            scanner::get_ecma_line_and_byte_offset_of_position(
                &self.source_file,
                current_token_info.token.loc.pos(),
            );

        if is_token_in_range {
            let range_has_error = (self.range_contains_error)(current_token_info.token.loc);
            // save previousRange since processRange will overwrite this value with current one
            let save_previous_range = self.previous_range.clone();
            line_action = self.process_range(
                current_token_info.token.clone(),
                token_start_line as i32,
                token_start_char as i32,
                parent.clone(),
                self.child_context_node
                    .clone()
                    .unwrap_or_else(|| parent.clone()),
                Some(dynamic_indenation.clone()),
            );
            // do not indent comments\token if token range overlaps with some error
            if !range_has_error {
                if line_action == LINE_ACTION_NONE {
                    // indent token only if end line of previous range does not match start line of the token
                    if save_previous_range != new_text_range_with_kind(0, 0, ast::Kind::Unknown) {
                        let prev_end_line = scanner::get_ecma_line_of_position(
                            &self.source_file,
                            save_previous_range.loc.end(),
                        );
                        indent_token = last_trivia_was_new_line
                            && token_start_line as i32 != prev_end_line as i32;
                    } else {
                        // When there's no previous range (first token), TS sets prevEndLine to undefined.
                        // tokenStart.line !== undefined is always true in JS, so indentToken = lastTriviaWasNewLine.
                        indent_token = last_trivia_was_new_line;
                    }
                } else {
                    indent_token = line_action == LINE_ACTION_LINE_ADDED;
                }
            }
        }

        if !current_token_info.trailing_trivia.is_empty() {
            self.previous_range_trivia_end =
                current_token_info.trailing_trivia.last().unwrap().loc.end();
            self.process_trivia(
                current_token_info.trailing_trivia.clone(),
                parent.clone(),
                self.child_context_node.clone(),
                Some(dynamic_indenation.clone()),
            );
        }

        if indent_token {
            let mut token_indentation = -1;
            if is_token_in_range && !(self.range_contains_error)(current_token_info.token.loc) {
                token_indentation = dynamic_indenation.get_indentation_for_token(
                    token_start_line as i32,
                    current_token_info.token.kind,
                    &container,
                    is_list_end_token,
                );
            }
            let mut indent_next_token_or_trivia = true;
            if !current_token_info.leading_trivia.is_empty() {
                let comment_indentation = dynamic_indenation.get_indentation_for_comment(
                    current_token_info.token.kind,
                    token_indentation,
                    &container,
                );
                indent_next_token_or_trivia = self.indent_trivia_items(
                    current_token_info.leading_trivia.clone(),
                    comment_indentation,
                    indent_next_token_or_trivia,
                    |worker, item| {
                        worker.insert_indentation(item.loc.pos(), comment_indentation, false)
                    },
                );
            }

            // indent token only if is it is in target range and does not overlap with any error ranges
            if token_indentation != -1 && indent_next_token_or_trivia {
                self.insert_indentation(
                    current_token_info.token.loc.pos(),
                    token_indentation,
                    line_action == LINE_ACTION_LINE_ADDED,
                );

                self.last_indented_line = token_start_line as i32;
                self.indentation_on_last_indented_line = token_indentation;
            }
        }

        self.scanner_mut().advance();

        self.child_context_node = Some(parent);
    }

    pub fn get_dynamic_indentation(
        &self,
        node: ast::Node,
        node_start_line: i32,
        indentation: i32,
        delta: i32,
    ) -> DynamicIndenter {
        DynamicIndenter {
            node,
            node_start_line,
            indentation,
            delta,
            options: self.formatting_context().options.clone(),
            source_file: self.source_file.share_readonly(),
        }
    }

    fn scanner(&self) -> &FormattingScanner {
        self.formatting_scanner.as_ref().unwrap()
    }

    fn scanner_mut(&mut self) -> &mut FormattingScanner {
        self.formatting_scanner.as_mut().unwrap()
    }

    fn read_token_info(&mut self, node: &ast::Node) -> TokenInfo {
        let store = self.source_file.store();
        let scanner = self.formatting_scanner.as_mut().unwrap();
        scanner.read_token_info(store, node)
    }

    fn formatting_context(&self) -> &FormattingContext {
        self.formatting_context.as_ref().unwrap()
    }

    fn formatting_context_mut(&mut self) -> &mut FormattingContext {
        self.formatting_context.as_mut().unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineAction(pub i32);

pub const LINE_ACTION_NONE: LineAction = LineAction(0);
pub const LINE_ACTION_LINE_ADDED: LineAction = LineAction(1);
pub const LINE_ACTION_LINE_REMOVED: LineAction = LineAction(2);

pub fn is_string_or_regular_expression_or_template_literal(kind: ast::Kind) -> bool {
    kind == ast::Kind::StringLiteral
        || kind == ast::Kind::RegularExpressionLiteral
        || ast::is_template_literal_kind(kind)
}

pub fn is_comment(kind: ast::Kind) -> bool {
    kind == ast::Kind::SingleLineCommentTrivia || kind == ast::Kind::MultiLineCommentTrivia
}

pub fn get_indentation_string(indentation: i32, options: lsutil::FormatCodeSettings) -> String {
    // go's `strings.Repeat` already has static, global caching for repeated tabs and spaces, so there's no need to cache here like in strada
    if !options.editor_settings.convert_tabs_to_spaces.is_true() {
        if options.editor_settings.tab_size == 0 {
            return String::new();
        }
        let tabs = (indentation as f64 / options.editor_settings.tab_size as f64).floor() as i32;
        let spaces = indentation - (tabs * options.editor_settings.tab_size);
        let mut res = "\t".repeat(tabs as usize);
        if spaces > 0 {
            res.push_str(&" ".repeat(spaces as usize));
        }

        res
    } else {
        " ".repeat(indentation as usize)
    }
}

pub fn create_text_change_from_start_length(
    start: i32,
    length: i32,
    new_text: String,
) -> core::TextChange {
    core::TextChange {
        new_text,
        text_range: core::new_text_range(start, start + length),
    }
}

pub struct DynamicIndenter {
    pub node: ast::Node,
    pub node_start_line: i32,
    pub indentation: i32,
    pub delta: i32,

    pub options: lsutil::FormatCodeSettings,
    pub source_file: ast::SourceFile,
}

impl Clone for DynamicIndenter {
    fn clone(&self) -> Self {
        Self {
            node: self.node,
            node_start_line: self.node_start_line,
            indentation: self.indentation,
            delta: self.delta,
            options: self.options.clone(),
            source_file: self.source_file.share_readonly(),
        }
    }
}

impl DynamicIndenter {
    pub fn get_indentation_for_comment(
        &self,
        kind: ast::Kind,
        token_indentation: i32,
        container: &ast::Node,
    ) -> i32 {
        match kind {
            // preceding comment to the token that closes the indentation scope inherits the indentation from the scope
            // ..  {
            //     // comment
            // }
            ast::Kind::CloseBraceToken
            | ast::Kind::CloseBracketToken
            | ast::Kind::CloseParenToken => {
                return self.indentation + self.get_delta(container);
            }
            _ => {}
        }
        if token_indentation != -1 {
            return token_indentation;
        }
        self.indentation
    }

    // if list end token is LessThanToken '>' then its delta should be explicitly suppressed
    // so that LessThanToken as a binary operator can still be indented.
    // foo.then
    //
    //  <
    //      number,
    //      string,
    //  >();
    //
    // vs
    // var a = xValue
    //
    //  > yValue;
    pub fn get_indentation_for_token(
        &self,
        line: i32,
        kind: ast::Kind,
        container: &ast::Node,
        suppress_delta: bool,
    ) -> i32 {
        if !suppress_delta && self.should_add_delta(line, kind, container) {
            return self.indentation + self.get_delta(container);
        }
        self.indentation
    }

    pub fn get_indentation(&self) -> i32 {
        self.indentation
    }

    pub fn get_delta(&self, child: &ast::Node) -> i32 {
        // Delta value should be zero when the node explicitly prevents indentation of the child node
        if node_will_indent_child(
            self.options.clone(),
            &self.node,
            Some(child),
            Some(&self.source_file),
            true,
        ) {
            return self.delta;
        }
        0
    }

    pub fn recompute_indentation(&mut self, line_added: bool, parent: Option<&ast::Node>) {
        let Some(parent) = parent else {
            return;
        };
        if should_indent_child_node(
            self.options.clone(),
            parent,
            Some(&self.node),
            &self.source_file,
            false,
        ) {
            if line_added {
                self.indentation += self.options.editor_settings.indent_size;
            } else {
                self.indentation -= self.options.editor_settings.indent_size;
            }
            if should_indent_child_node(
                self.options.clone(),
                &self.node,
                None,
                &self.source_file,
                false,
            ) {
                self.delta = self.options.editor_settings.indent_size;
            } else {
                self.delta = 0;
            }
        }
    }

    pub fn should_add_delta(&self, line: i32, kind: ast::Kind, container: &ast::Node) -> bool {
        let store = self.source_file.store();
        match kind {
            // open and close brace, 'else' and 'while' (in do statement) tokens has indentation of the parent
            ast::Kind::OpenBraceToken
            | ast::Kind::CloseBraceToken
            | ast::Kind::CloseParenToken
            | ast::Kind::ElseKeyword
            | ast::Kind::WhileKeyword
            | ast::Kind::AtToken => return false,
            ast::Kind::SlashToken | ast::Kind::GreaterThanToken => match store.kind(*container) {
                ast::Kind::JsxOpeningElement
                | ast::Kind::JsxClosingElement
                | ast::Kind::JsxSelfClosingElement => return false,
                _ => {}
            },
            ast::Kind::OpenBracketToken | ast::Kind::CloseBracketToken => {
                if store.kind(*container) != ast::Kind::MappedType {
                    return false;
                }
            }
            _ => {}
        }
        // if token line equals to the line of containing node (this is a first token in the node) - use node indentation
        self.node_start_line != line
            &&
            // if this token is the first token following the list of decorators, we do not need to indent
            !(ast::has_decorators(self.source_file.store(), self.node)
                && kind == get_first_non_decorator_token_of_node(self.source_file.store(), &self.node))
    }
}

pub fn get_first_non_decorator_token_of_node(store: &ast::AstStore, node: &ast::Node) -> ast::Kind {
    if ast::can_have_modifiers(store, *node) {
        let decorators = store.modifier_nodes(*node);
        let start = usize::try_from(core::find_index(&decorators, |node| {
            ast::is_decorator(store, *node)
        }))
        .expect("decorator modifier expected");
        if let Some(modifier) = decorators[start..]
            .iter()
            .find(|node| ast::is_modifier(store, **node))
        {
            return store.kind(*modifier);
        }
    }

    match store.kind(*node) {
        ast::Kind::ClassDeclaration => ast::Kind::ClassKeyword,
        ast::Kind::InterfaceDeclaration => ast::Kind::InterfaceKeyword,
        ast::Kind::FunctionDeclaration => ast::Kind::FunctionKeyword,
        ast::Kind::EnumDeclaration => ast::Kind::EnumDeclaration,
        ast::Kind::GetAccessor => ast::Kind::GetKeyword,
        ast::Kind::SetAccessor => ast::Kind::SetKeyword,
        ast::Kind::MethodDeclaration => {
            if store.asterisk_token(*node).is_some() {
                return ast::Kind::AsteriskToken;
            }
            ast::get_name_of_declaration(store, Some(*node))
                .map_or(ast::Kind::Unknown, |name| store.kind(name))
        }
        ast::Kind::PropertyDeclaration | ast::Kind::Parameter => {
            ast::get_name_of_declaration(store, Some(*node))
                .map_or(ast::Kind::Unknown, |name| store.kind(name))
        }
        _ => ast::Kind::Unknown,
    }
}

pub fn is_white_space_single_line(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\u{000b}' | '\u{000c}')
}
