use std::collections::HashMap;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_collections as collections;
use ts_core as core;
use ts_format as format;
use ts_lsproto as lsproto;
use ts_printer as printer;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::lsconv;
use crate::lsutil;

use super::delete::{delete_declaration, positions_are_on_same_line};
use super::trackerimpl::{has_comments_before_line_break, need_semicolon_between};

#[derive(Clone, Debug, Default)]
pub struct NodeOptions {
    // Text to be inserted before the new node
    pub prefix: String,

    // Text to be inserted after the new node
    pub suffix: String,

    // Text of inserted node will be formatted with this indentation, otherwise indentation will be inferred from the old node
    pub indentation: Option<i32>,

    // Text of inserted node will be formatted with this delta, otherwise delta will be inferred from the new node kind
    pub delta: Option<i32>,

    pub leading_trivia_option: LeadingTriviaOption,
    pub trailing_trivia_option: TrailingTriviaOption,
    pub joiner: String,
}

pub type LeadingTriviaOption = i32;

pub const LEADING_TRIVIA_OPTION_NONE: LeadingTriviaOption = 0;
pub const LEADING_TRIVIA_OPTION_EXCLUDE: LeadingTriviaOption = 1;
pub const LEADING_TRIVIA_OPTION_INCLUDE_ALL: LeadingTriviaOption = 2;
pub const LEADING_TRIVIA_OPTION_START_LINE: LeadingTriviaOption = 4;

pub type TrailingTriviaOption = i32;

pub const TRAILING_TRIVIA_OPTION_NONE: TrailingTriviaOption = 0;
pub const TRAILING_TRIVIA_OPTION_EXCLUDE: TrailingTriviaOption = 1;
pub const TRAILING_TRIVIA_OPTION_EXCLUDE_WHITESPACE: TrailingTriviaOption = 2;
pub const TRAILING_TRIVIA_OPTION_INCLUDE: TrailingTriviaOption = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackerEditKind {
    Text = 1,
    Remove = 2,
    ReplaceWithSingleNode = 3,
    ReplaceWithMultipleNodes = 4,
}

pub struct TrackerEdit<'a> {
    pub source_file: &'a ast::SourceFile,
    pub kind: TrackerEditKind,
    pub range: lsproto::Range,

    pub new_text: String, // kind == text

    pub node: Option<ast::Node>, // single
    pub nodes: Vec<ast::Node>,   // multiple
    pub options: NodeOptions,
}

#[derive(Clone, Copy)]
pub struct NodesInsertedAtStartState<'a> {
    pub node: ast::Node,
    pub source_file: &'a ast::SourceFile,
}

pub struct Tracker<'a> {
    // initialized with
    pub format_settings: lsutil::FormatCodeSettings,
    pub new_line: String,
    pub converters: &'a lsconv::Converters,
    pub ctx: format::Context,
    pub emit_context: printer::EmitContext,

    pub node_factory: ast::NodeFactory,

    pub changes: collections::MultiMap<checker::SourceFileIdentity, TrackerEdit<'a>>,
    pub deleted_nodes: Vec<DeletedNode<'a>>,
    pub nodes_with_insertions_at_start: HashMap<ast::Node, NodesInsertedAtStartState<'a>>,

    // created during call to getChanges
    pub writer: Option<printer::ChangeTrackerWriter>,
    // printer
}

#[derive(Clone, Copy)]
pub struct DeletedNode<'a> {
    pub source_file: &'a ast::SourceFile,
    pub node: ast::Node,
}

pub fn new_tracker<'a>(
    _ctx: core::Context,
    compiler_options: &core::CompilerOptions,
    format_options: lsutil::FormatCodeSettings,
    converters: &'a lsconv::Converters,
) -> Tracker<'a> {
    let emit_context = printer::new_emit_context();
    let new_line = compiler_options
        .new_line
        .get_new_line_character()
        .to_string();
    let ctx = format::with_format_code_settings(
        format::Context::default(),
        format_options.clone(),
        new_line.clone(),
    );
    Tracker {
        emit_context,
        node_factory: ast::NodeFactory::default(),
        changes: collections::MultiMap::new(),
        ctx,
        converters,
        format_settings: format_options,
        new_line,
        deleted_nodes: Vec::new(),
        nodes_with_insertions_at_start: HashMap::new(),
        writer: None,
    }
}

impl<'a> Tracker<'a> {
    fn import_new_node(&mut self, source_file: &'a ast::SourceFile, node: ast::Node) -> ast::Node {
        if node.store_id() == self.node_factory.store().store_id()
            || node.store_id() == source_file.store().store_id()
        {
            return node;
        }

        let emit_store = self.emit_context.factory.node_factory.store();
        if node.store_id() == emit_store.store_id() {
            return self
                .node_factory
                .deep_clone_node_from_store_preserve_location(emit_store, node);
        }

        if let Some(source_file) = self.emit_context.source_file_handle_for_node(node) {
            return self
                .node_factory
                .deep_clone_node_from_store_preserve_location(source_file.store(), node);
        }

        panic!(
            "change tracker cannot import generated node from AST store {:?}",
            node.store_id()
        );
    }

    fn import_new_nodes(
        &mut self,
        source_file: &'a ast::SourceFile,
        nodes: Vec<ast::Node>,
    ) -> Vec<ast::Node> {
        nodes
            .into_iter()
            .map(|node| self.import_new_node(source_file, node))
            .collect()
    }

    fn store_for_inserted_node(
        &self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
    ) -> &ast::AstStore {
        if node.store_id() == self.node_factory.store().store_id() {
            self.node_factory.store()
        } else {
            debug_assert_eq!(node.store_id(), source_file.store().store_id());
            source_file.store()
        }
    }

    // GetChanges returns the accumulated text edits.
    // Note: after calling this, the Tracker object must be discarded!
    pub fn get_changes(&mut self) -> HashMap<String, Vec<lsproto::TextEdit>> {
        self.finish_delete_declarations();
        self.finish_nodes_with_insertions_at_start();
        let changes = self.get_text_changes_from_changes();
        // !!! changes for new files
        changes
    }

    pub fn replace_node(
        &mut self,
        source_file: &'a ast::SourceFile,
        old_node: ast::Node,
        new_node: ast::Node,
        options: Option<NodeOptions>,
    ) {
        let options = options.unwrap_or(NodeOptions {
            // defaults to `useNonAdjustedPositions`
            leading_trivia_option: LEADING_TRIVIA_OPTION_EXCLUDE,
            trailing_trivia_option: TRAILING_TRIVIA_OPTION_EXCLUDE,
            ..Default::default()
        });
        self.replace_range(
            source_file,
            self.get_adjusted_range(
                source_file,
                old_node,
                old_node,
                options.leading_trivia_option,
                options.trailing_trivia_option,
            ),
            new_node,
            options,
        );
    }

    pub fn replace_node_with_nodes(
        &mut self,
        source_file: &'a ast::SourceFile,
        old_node: ast::Node,
        new_nodes: Vec<ast::Node>,
        options: Option<NodeOptions>,
    ) {
        let options = options.unwrap_or(NodeOptions {
            leading_trivia_option: LEADING_TRIVIA_OPTION_EXCLUDE,
            trailing_trivia_option: TRAILING_TRIVIA_OPTION_EXCLUDE,
            ..Default::default()
        });
        self.replace_range_with_nodes(
            source_file,
            self.get_adjusted_range(
                source_file,
                old_node,
                old_node,
                options.leading_trivia_option,
                options.trailing_trivia_option,
            ),
            new_nodes,
            options,
        );
    }

    pub fn replace_range(
        &mut self,
        source_file: &'a ast::SourceFile,
        lsp_range: lsproto::Range,
        new_node: ast::Node,
        options: NodeOptions,
    ) {
        let new_node = self.import_new_node(source_file, new_node);
        self.changes.add(
            checker::SourceFileIdentity::from_source_file(source_file),
            TrackerEdit {
                source_file,
                kind: TrackerEditKind::ReplaceWithSingleNode,
                range: lsp_range,
                options,
                node: Some(new_node),
                nodes: Vec::new(),
                new_text: String::new(),
            },
        );
    }

    pub fn replace_range_with_text(
        &mut self,
        source_file: &'a ast::SourceFile,
        lsp_range: lsproto::Range,
        text: &str,
    ) {
        self.changes.add(
            checker::SourceFileIdentity::from_source_file(source_file),
            TrackerEdit {
                source_file,
                kind: TrackerEditKind::Text,
                range: lsp_range,
                new_text: text.to_string(),
                node: None,
                nodes: Vec::new(),
                options: NodeOptions::default(),
            },
        );
    }

    pub fn replace_range_with_nodes(
        &mut self,
        source_file: &'a ast::SourceFile,
        lsp_range: lsproto::Range,
        new_nodes: Vec<ast::Node>,
        options: NodeOptions,
    ) {
        if new_nodes.len() == 1 {
            self.replace_range(source_file, lsp_range, new_nodes[0], options);
            return;
        }
        let new_nodes = self.import_new_nodes(source_file, new_nodes);
        self.changes.add(
            checker::SourceFileIdentity::from_source_file(source_file),
            TrackerEdit {
                source_file,
                kind: TrackerEditKind::ReplaceWithMultipleNodes,
                range: lsp_range,
                nodes: new_nodes,
                options,
                node: None,
                new_text: String::new(),
            },
        );
    }

    pub fn insert_text(
        &mut self,
        source_file: &'a ast::SourceFile,
        pos: lsproto::Position,
        text: &str,
    ) {
        self.replace_range_with_text(
            source_file,
            lsproto::Range {
                start: pos,
                end: pos,
            },
            text,
        );
    }

    pub fn insert_node_at(
        &mut self,
        source_file: &'a ast::SourceFile,
        pos: core::TextPos,
        new_node: ast::Node,
        options: NodeOptions,
    ) {
        let ls_pos = self
            .converters
            .position_to_line_and_character(source_file, pos);
        self.replace_range(
            source_file,
            lsproto::Range {
                start: ls_pos,
                end: ls_pos,
            },
            new_node,
            options,
        );
    }

    pub fn insert_nodes_at(
        &mut self,
        source_file: &'a ast::SourceFile,
        pos: core::TextPos,
        new_nodes: Vec<ast::Node>,
        options: NodeOptions,
    ) {
        let ls_pos = self
            .converters
            .position_to_line_and_character(source_file, pos);
        self.replace_range_with_nodes(
            source_file,
            lsproto::Range {
                start: ls_pos,
                end: ls_pos,
            },
            new_nodes,
            options,
        );
    }

    pub fn insert_node_after(
        &mut self,
        source_file: &'a ast::SourceFile,
        after: ast::Node,
        new_node: ast::Node,
    ) {
        let new_node = self.import_new_node(source_file, new_node);
        let end_position = self.end_pos_for_insert_node_after(source_file, after, new_node);
        let options = self.get_insert_node_after_options(source_file, after);
        self.insert_node_at(source_file, end_position, new_node, options);
    }

    pub fn insert_nodes_after(
        &mut self,
        source_file: &'a ast::SourceFile,
        after: ast::Node,
        new_nodes: Vec<ast::Node>,
    ) {
        let new_nodes = self.import_new_nodes(source_file, new_nodes);
        let end_position = self.end_pos_for_insert_node_after(source_file, after, new_nodes[0]);
        let options = self.get_insert_node_after_options(source_file, after);
        self.insert_nodes_at(source_file, end_position, new_nodes, options);
    }

    pub fn insert_node_before(
        &mut self,
        source_file: &'a ast::SourceFile,
        before: ast::Node,
        new_node: ast::Node,
        blank_line_between: bool,
        leading_trivia_option: LeadingTriviaOption,
    ) {
        let new_node = self.import_new_node(source_file, new_node);
        let pos =
            self.get_adjusted_start_position(source_file, before, leading_trivia_option, false);
        let options = self.get_options_for_insert_node_before(
            source_file,
            before,
            new_node,
            blank_line_between,
        );
        self.insert_node_at(source_file, core::TextPos(pos), new_node, options);
    }

    // TryInsertTypeAnnotation inserts a type annotation after the appropriate position on a node
    // (after the close paren for function-like, after the name/exclamation/question for variable-like).
    // Returns true if successful.
    pub fn try_insert_type_annotation(
        &mut self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
        type_node: ast::Node,
    ) -> bool {
        let store = source_file.store();
        let mut end_pos = None;
        if ast::is_function_like(store, Some(node)) {
            end_pos =
                astnav::find_child_of_kind_info(node, ast::Kind::CloseParenToken, source_file)
                    .map(|token| token.loc.end());
            if end_pos.is_none() {
                if !ast::is_arrow_function(store, node) {
                    return false;
                }
                // If no `)`, is an arrow function `x => x`, so use the end of the first parameter
                let Some(params) = store.parameters(node) else {
                    return false;
                };
                if params.is_empty() {
                    return false;
                }
                end_pos = params.first().map(|param| store.loc(param).end());
            }
        } else {
            match store.kind(node) {
                ast::Kind::VariableDeclaration => {
                    end_pos = store
                        .exclamation_token(node)
                        .map(|node| store.loc(node).end());
                }
                ast::Kind::PropertySignature => {
                    end_pos = store.postfix_token(node).map(|node| store.loc(node).end());
                }
                ast::Kind::PropertyDeclaration => {
                    end_pos = store.postfix_token(node).map(|node| store.loc(node).end());
                }
                ast::Kind::Parameter => {
                    end_pos = store.question_token(node).map(|node| store.loc(node).end());
                }
                _ => {}
            }
            if end_pos.is_none() {
                end_pos = store.name(node).map(|node| store.loc(node).end());
            }
        }
        let Some(end_pos) = end_pos else {
            return false;
        };
        self.insert_node_at(
            source_file,
            core::TextPos(end_pos),
            type_node,
            NodeOptions {
                prefix: ": ".to_string(),
                ..Default::default()
            },
        );
        true
    }

    // ParenthesizeArrowParameters wraps the parameters of a paren-less arrow function in `(` and `)`.
    // This is a no-op if the arrow function already has parens.
    pub fn parenthesize_arrow_parameters(
        &mut self,
        source_file: &'a ast::SourceFile,
        arrow_func: ast::Node,
    ) {
        if astnav::has_child_of_kind(arrow_func, ast::Kind::CloseParenToken, source_file) {
            return;
        }
        let store = source_file.store();
        let Some(params) = store.parameters(arrow_func) else {
            return;
        };
        if params.is_empty() {
            return;
        }
        let first_param = params.first().expect("non-empty parameter list");
        let last_param = params.last().expect("non-empty parameter list");
        let start_pos = astnav::get_start_of_node(first_param, source_file);
        self.insert_text(
            source_file,
            self.converters
                .position_to_line_and_character(source_file, core::TextPos(start_pos)),
            "(",
        );
        self.insert_text(
            source_file,
            self.converters.position_to_line_and_character(
                source_file,
                core::TextPos(store.loc(last_param).end()),
            ),
            ")",
        );
    }

    // InsertModifierBefore inserts a modifier token (like 'type') before a node with a trailing space.
    pub fn insert_modifier_before(
        &mut self,
        source_file: &'a ast::SourceFile,
        modifier: ast::Kind,
        before: ast::Node,
    ) {
        let pos = astnav::get_start_of_node(before, source_file);
        let token = self.node_factory.new_token(modifier);
        let loc = core::new_text_range(pos, pos);
        let parent = source_file.store().parent(before);
        self.node_factory.place_change_tracker_node(token, loc);
        self.node_factory.link_change_tracker_parent(token, parent);
        self.insert_node_at(
            source_file,
            core::TextPos(pos),
            token,
            NodeOptions {
                suffix: " ".to_string(),
                ..Default::default()
            },
        );
    }

    // Delete queues a node for deletion with smart handling of list items, imports, etc.
    // The actual deletion happens in finishDeleteDeclarations during GetChanges.
    pub(crate) fn delete(&mut self, source_file: &'a ast::SourceFile, node: ast::Node) {
        self.deleted_nodes.push(DeletedNode { source_file, node });
    }

    // DeleteRange deletes a text range from the source file.
    pub fn delete_range(&mut self, source_file: &'a ast::SourceFile, text_range: core::TextRange) {
        let lsp_range = self.converters.to_lsp_range(source_file, text_range);
        self.replace_range_with_text(source_file, lsp_range, "");
    }

    // DeleteNode deletes a node immediately with specified trivia options.
    // Stop! Consider using Delete instead, which has logic for deleting nodes from delimited lists.
    pub fn delete_node(
        &mut self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
        leading_trivia: LeadingTriviaOption,
        trailing_trivia: TrailingTriviaOption,
    ) {
        let range =
            self.get_adjusted_range(source_file, node, node, leading_trivia, trailing_trivia);
        self.replace_range_with_text(source_file, range, "");
    }

    // DeleteNodeRange deletes a range of nodes with specified trivia options.
    pub fn delete_node_range(
        &mut self,
        source_file: &'a ast::SourceFile,
        start_node: ast::Node,
        end_node: ast::Node,
        leading_trivia: LeadingTriviaOption,
        trailing_trivia: TrailingTriviaOption,
    ) {
        let start_position =
            self.get_adjusted_start_position(source_file, start_node, leading_trivia, false);
        let end_position = self.get_adjusted_end_position(source_file, end_node, trailing_trivia);
        let start_pos = self
            .converters
            .position_to_line_and_character(source_file, core::TextPos(start_position));
        let end_pos = self
            .converters
            .position_to_line_and_character(source_file, core::TextPos(end_position));
        self.replace_range_with_text(
            source_file,
            lsproto::Range {
                start: start_pos,
                end: end_pos,
            },
            "",
        );
    }

    pub fn delete_token_info_range(
        &mut self,
        source_file: &'a ast::SourceFile,
        start_token: astnav::TokenInfo,
        end_token: astnav::TokenInfo,
        leading_trivia: LeadingTriviaOption,
        trailing_trivia: TrailingTriviaOption,
    ) {
        let range = self.get_adjusted_token_info_range(
            source_file,
            start_token,
            end_token,
            leading_trivia,
            trailing_trivia,
        );
        self.replace_range_with_text(source_file, range, "");
    }

    // finishDeleteDeclarations processes all queued deletions with smart handling for lists and trailing commas.
    pub fn finish_delete_declarations(&mut self) {
        let mut deleted_nodes_in_lists: HashMap<ast::Node, ast::Node> = HashMap::new();

        for deleted in self.deleted_nodes.clone() {
            // Skip if this node is contained within another deleted node
            let mut is_contained = false;
            for other in &self.deleted_nodes {
                if std::ptr::eq(other.source_file, deleted.source_file)
                    && other.node != deleted.node
                    && range_contains_range_exclusive(
                        deleted.source_file.store(),
                        other.node,
                        deleted.node,
                    )
                {
                    is_contained = true;
                    break;
                }
            }
            if is_contained {
                continue;
            }

            delete_declaration(
                self,
                &mut deleted_nodes_in_lists,
                deleted.source_file,
                deleted.node,
            );
        }

        // Handle trailing commas for last elements in lists
        for node in deleted_nodes_in_lists.values().copied().collect::<Vec<_>>() {
            let source_file = self
                .deleted_nodes
                .iter()
                .find(|deleted| deleted.node == node)
                .map(|deleted| deleted.source_file)
                .expect("deleted list node should have a queued source file");
            let list = format::get_containing_list(&node, source_file);
            let Some(list) = list else {
                continue;
            };
            let list_nodes = list.iter().collect::<Vec<_>>();
            if node != list_nodes[list_nodes.len() - 1] {
                continue;
            }

            let mut last_non_deleted_index = -1;
            for i in (0..list_nodes.len() - 1).rev() {
                if !deleted_nodes_in_lists.contains_key(&list_nodes[i]) {
                    last_non_deleted_index = i as i32;
                    break;
                }
            }

            if last_non_deleted_index != -1 {
                let index = last_non_deleted_index as usize;
                let start_pos = self.converters.position_to_line_and_character(
                    source_file,
                    core::TextPos(source_file.store().loc(list_nodes[index]).end()),
                );
                let delete_start =
                    self.start_position_to_delete_node_in_list(source_file, list_nodes[index + 1]);
                let end_pos = self
                    .converters
                    .position_to_line_and_character(source_file, core::TextPos(delete_start));
                self.replace_range_with_text(
                    source_file,
                    lsproto::Range {
                        start: start_pos,
                        end: end_pos,
                    },
                    "",
                );
            }
        }
    }

    pub fn end_pos_for_insert_node_after(
        &mut self,
        source_file: &'a ast::SourceFile,
        after: ast::Node,
        new_node: ast::Node,
    ) -> core::TextPos {
        let inserted_store = self.store_for_inserted_node(source_file, new_node);
        if need_semicolon_between(source_file.store(), after, inserted_store, new_node)
            && source_file.text().as_bytes()[source_file.store().loc(after).end() as usize - 1]
                != b';'
        {
            // check if previous statement ends with semicolon
            // if not - insert semicolon to preserve the code from changing the meaning due to ASI
            let end_pos = self.converters.position_to_line_and_character(
                source_file,
                core::TextPos(source_file.store().loc(after).end()),
            );
            let semicolon = self.node_factory.new_token(ast::Kind::SemicolonToken);
            let end = source_file.store().loc(after).end();
            let loc = core::new_text_range(end, end);
            let parent = source_file.store().parent(after);
            self.node_factory.place_change_tracker_node(semicolon, loc);
            self.node_factory
                .link_change_tracker_parent(semicolon, parent);
            self.replace_range(
                source_file,
                lsproto::Range {
                    start: end_pos,
                    end: end_pos,
                },
                semicolon,
                NodeOptions::default(),
            );
        }
        core::TextPos(self.get_adjusted_end_position(
            source_file,
            after,
            TRAILING_TRIVIA_OPTION_NONE,
        ))
    }

    /**
     * This function should be used to insert nodes in lists when nodes don't carry separators as the part of the node range,
     * i.e. arguments in arguments lists, parameters in parameter lists etc.
     * Note that separators are part of the node in statements and class elements.
     */
    pub fn insert_node_in_list_after(
        &mut self,
        source_file: &'a ast::SourceFile,
        after: ast::Node,
        new_node: ast::Node,
        containing_list: Option<ast::SourceNodeList<'a>>,
    ) {
        let containing_list =
            containing_list.or_else(|| format::get_containing_list(&after, source_file));
        let Some(containing_list) = containing_list else {
            // Debug.fail("node is not a list element")
            return;
        };
        let list_nodes = containing_list.iter().collect::<Vec<_>>();
        let Some(index) = list_nodes.iter().position(|node| *node == after) else {
            return;
        };
        let store = source_file.store();
        let end = store.loc(after).end();
        if index != list_nodes.len() - 1 {
            // any element except the last one
            // use next sibling as an anchor
            let next_token = astnav::get_token_at_position(source_file, end);
            if next_token.is_some_and(|next_token| is_separator(store, after, Some(next_token))) {
                let next_token = next_token.unwrap();
                // for list
                // a, b, c
                // create change for adding 'e' after 'a' as
                // - find start of next element after a (it is b)
                // - use next element start as start and end position in final change
                // - build text of change by formatting the text of node + whitespace trivia of b

                // in multiline case it will work as
                //   a,
                //   b,
                //   c,
                // result - '*' denotes leading trivia that will be inserted after new text (displayed as '#')
                //   a,
                //   insertedtext<separator>#
                // ###b,
                //   c,
                let next_node = list_nodes[index + 1];
                let options = scanner::SkipTriviaOptions {
                    stop_after_line_break: false,
                    stop_at_comments: true,
                };
                let start_pos = scanner::skip_trivia_ex(
                    source_file.text(),
                    store.loc(next_node).pos() as usize,
                    Some(&options),
                );

                // write separator and leading trivia of the next element as suffix
                let suffix = scanner::token_to_string(store.kind(next_token))
                    + &source_file.text()[store.loc(next_token).end() as usize..start_pos as usize];
                self.insert_nodes_at(
                    source_file,
                    core::TextPos(start_pos as i32),
                    vec![new_node],
                    NodeOptions {
                        suffix,
                        ..Default::default()
                    },
                );
            }
            return;
        }

        let after_start = astnav::get_start_of_node(after, source_file);
        let after_start_line_position =
            format::get_line_start_position_for_position(after_start, source_file);

        // insert element after the last element in the list that has more than one item
        // pick the element preceding the after element to:
        // - pick the separator
        // - determine if list is a multiline
        let mut multiline_list = false;

        // if list has only one element then we'll format is as multiline if node has comment in trailing trivia, or as singleline otherwise
        // i.e. var x = 1 // this is x
        //     | new element will be inserted at this position
        let mut separator = ast::Kind::CommaToken; // SyntaxKind.CommaToken | SyntaxKind.SemicolonToken
        if list_nodes.len() != 1 {
            // otherwise, if list has more than one element, pick separator from the list
            let token_before_insert_position =
                astnav::find_preceding_token(source_file, store.loc(after).pos());
            separator = if is_separator(source_file.store(), after, token_before_insert_position) {
                store.kind(token_before_insert_position.unwrap())
            } else {
                ast::Kind::CommaToken
            };
            // determine if list is multiline by checking lines of after element and element that precedes it.
            let after_minus_one_start_line_position = format::get_line_start_position_for_position(
                astnav::get_start_of_node(list_nodes[index - 1], source_file),
                source_file,
            );
            multiline_list = after_minus_one_start_line_position != after_start_line_position;
        }
        if has_comments_before_line_break(source_file.text(), end)
            || !positions_are_on_same_line(
                containing_list.pos(),
                containing_list.end(),
                source_file,
            )
        {
            // in this case we'll always treat containing list as multiline
            multiline_list = true;
        }
        if multiline_list {
            // insert separator immediately following the 'after' node to preserve comments in trailing trivia
            let separator_token = self.node_factory.new_token(separator);
            let separator_string = scanner::token_to_string(separator);
            let loc = core::new_text_range(end, end + separator_string.len() as i32);
            let parent = source_file.store().parent(after);
            self.node_factory
                .place_change_tracker_node(separator_token, loc);
            self.node_factory
                .link_change_tracker_parent(separator_token, parent);
            let end_pos = self
                .converters
                .position_to_line_and_character(source_file, core::TextPos(end));
            self.replace_range(
                source_file,
                lsproto::Range {
                    start: end_pos,
                    end: end_pos,
                },
                separator_token,
                NodeOptions::default(),
            );
            // use the same indentation as 'after' item
            let indentation = find_indentation_column(
                source_file.text(),
                after_start_line_position,
                after_start,
                self.format_settings.editor_settings.tab_size,
            );
            let options = scanner::SkipTriviaOptions {
                stop_after_line_break: true,
                stop_at_comments: false,
            };
            // insert element before the line break on the line that contains 'after' element
            let mut insert_pos =
                scanner::skip_trivia_ex(source_file.text(), end as usize, Some(&options)) as i32;
            // find position before "\n" or "\r\n"
            while insert_pos != end
                && stringutil::is_line_break(
                    source_file.text().as_bytes()[insert_pos as usize - 1] as char,
                )
            {
                insert_pos -= 1;
            }
            let insert_ls_pos = self
                .converters
                .position_to_line_and_character(source_file, core::TextPos(insert_pos));
            self.replace_range(
                source_file,
                lsproto::Range {
                    start: insert_ls_pos,
                    end: insert_ls_pos,
                },
                new_node,
                NodeOptions {
                    indentation: Some(indentation),
                    prefix: self.new_line.clone(),
                    ..Default::default()
                },
            );
        } else {
            let separator_string = scanner::token_to_string(separator);
            let end_pos = self
                .converters
                .position_to_line_and_character(source_file, core::TextPos(end));
            self.replace_range(
                source_file,
                lsproto::Range {
                    start: end_pos,
                    end: end_pos,
                },
                new_node,
                NodeOptions {
                    prefix: separator_string + " ",
                    ..Default::default()
                },
            );
        }
    }

    // InsertImportSpecifierAtIndex inserts a new import specifier at the specified index in a NamedImports list
    pub fn insert_import_specifier_at_index(
        &mut self,
        source_file: &'a ast::SourceFile,
        new_specifier: ast::Node,
        named_imports: ast::Node,
        index: usize,
    ) {
        let store = source_file.store();
        let elements = store
            .elements(named_imports)
            .expect("named imports should have elements")
            .iter()
            .collect::<Vec<_>>();

        let mut prev_specifier = None;
        if index > 0 && index - 1 < elements.len() {
            prev_specifier = Some(elements[index - 1]);
        }
        if let Some(prev_specifier) = prev_specifier {
            self.insert_node_in_list_after(source_file, prev_specifier, new_specifier, None);
        } else {
            self.insert_node_before(
                source_file,
                elements[0],
                new_specifier,
                !positions_are_on_same_line(
                    astnav::get_start_of_node(elements[0], source_file),
                    {
                        let import_clause = store
                            .parent(named_imports)
                            .expect("named imports should have import clause parent");
                        let import_declaration = store
                            .parent(import_clause)
                            .expect("import clause should have import declaration parent");
                        astnav::get_start_of_node(import_declaration, source_file)
                    },
                    source_file,
                ),
                LEADING_TRIVIA_OPTION_NONE,
            );
        }
    }

    pub fn insert_at_top_of_file(
        &mut self,
        source_file: &'a ast::SourceFile,
        insert: Vec<&'a ast::Statement>,
        blank_line_between: bool,
    ) {
        if insert.is_empty() {
            return;
        }

        let pos = self.get_insertion_position_at_source_file_top(source_file);
        let mut options = NodeOptions::default();
        if pos != 0 {
            options.prefix = self.new_line.clone();
        }
        if source_file.text().is_empty()
            || !stringutil::is_line_break(source_file.text().as_bytes()[pos as usize] as char)
        {
            options.suffix = self.new_line.clone();
        }
        if blank_line_between {
            options.suffix.push_str(&self.new_line);
        }

        if insert.len() == 1 {
            self.insert_node_at(source_file, core::TextPos(pos), *insert[0], options);
        } else {
            self.insert_nodes_at(
                source_file,
                core::TextPos(pos),
                insert.into_iter().copied().collect(),
                options,
            );
        }
    }

    pub fn insert_member_at_start(
        &mut self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
        new_element: ast::Node,
    ) {
        self.insert_node_at_start_worker(source_file, node, new_element);
    }

    pub fn insert_node_at_start_worker(
        &mut self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
        new_element: ast::Node,
    ) {
        let mut indentation = self.try_compute_indentation_from_existing_members(source_file, node);
        if indentation < 0 {
            indentation = self.try_compute_indentation_for_new_member(source_file, node);
        }

        let members = get_members_or_properties(source_file.store(), node);
        if members.is_none() {
            return;
        }
        let members = members.unwrap();

        let options = self.get_insert_node_at_start_insert_options(source_file, node, indentation);
        self.insert_node_at(
            source_file,
            core::TextPos(members.pos()),
            new_element,
            options,
        );
    }

    pub fn try_compute_indentation_for_new_member(
        &self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
    ) -> i32 {
        let node_start = astnav::get_start_of_node(node, source_file);
        let line_start = format::get_line_start_position_for_position(node_start, source_file);

        let mut tab_size = self.format_settings.editor_settings.tab_size;
        if tab_size <= 0 {
            tab_size = 4;
        }

        let mut indent_size = self.format_settings.editor_settings.indent_size;
        if indent_size <= 0 {
            indent_size = 4;
        }
        find_indentation_column(source_file.text(), line_start, node_start, tab_size).max(0)
            + indent_size
    }

    pub fn try_compute_indentation_from_existing_members(
        &self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
    ) -> i32 {
        let members = get_members_or_properties(source_file.store(), node);
        if members.is_none() {
            return -1;
        }
        let members = members.unwrap();

        let mut indentation = -1;
        let text = source_file.text();
        let mut tab_size = self.format_settings.editor_settings.tab_size;
        let mut last = node;

        if tab_size <= 0 {
            tab_size = 4;
        }

        for member in members {
            if printer::range_start_positions_are_on_same_line(
                source_file.store().loc(last),
                source_file.store().loc(member),
                source_file,
            ) {
                return -1;
            }

            let member_start = astnav::get_start_of_node(member, source_file);
            let line_start =
                format::get_line_start_position_for_position(member_start, source_file);
            let column = find_indentation_column(text, line_start, member_start, tab_size);
            if column < 0 {
                return -1;
            }

            if indentation >= 0 {
                if indentation != column {
                    return -1;
                }
                last = member;
                continue;
            }

            indentation = column;
            last = member;
        }

        indentation
    }

    pub fn get_insert_node_after_options(
        &self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
    ) -> NodeOptions {
        let store = source_file.store();
        let new_line_char = self.new_line.clone();
        let mut options;
        match store.kind(node) {
            ast::Kind::Parameter => {
                // default opts
                options = NodeOptions::default();
            }
            ast::Kind::ClassDeclaration | ast::Kind::ModuleDeclaration => {
                options = NodeOptions {
                    prefix: new_line_char.clone(),
                    suffix: new_line_char.clone(),
                    ..Default::default()
                };
            }

            ast::Kind::VariableDeclaration | ast::Kind::StringLiteral | ast::Kind::Identifier => {
                options = NodeOptions {
                    prefix: ", ".to_string(),
                    ..Default::default()
                };
            }

            ast::Kind::PropertyAssignment => {
                options = NodeOptions {
                    suffix: ",".to_string() + &new_line_char,
                    ..Default::default()
                };
            }

            ast::Kind::ExportKeyword => {
                options = NodeOptions {
                    prefix: " ".to_string(),
                    ..Default::default()
                };
            }

            _ => {
                if !(ast::is_statement(store, node) || ast::is_class_or_type_element(store, &node))
                {
                    // Else we haven't handled this kind of node yet -- add it
                    panic!(
                        "unimplemented node type {} in changeTracker.getInsertNodeAfterOptions",
                        store.kind(node).to_string()
                    );
                }
                options = NodeOptions {
                    suffix: new_line_char,
                    ..Default::default()
                };
            }
        }
        if store.loc(node).end() == source_file.end() && ast::is_statement(store, node) {
            options.prefix = self.new_line.clone() + &options.prefix;
        }

        options
    }

    pub fn get_options_for_insert_node_before(
        &self,
        source_file: &'a ast::SourceFile,
        before: ast::Node,
        inserted: ast::Node,
        blank_line_between: bool,
    ) -> NodeOptions {
        let store = source_file.store();
        let inserted_store = self.store_for_inserted_node(source_file, inserted);
        if ast::is_statement(store, before) || ast::is_class_or_type_element(store, &before) {
            if blank_line_between {
                return NodeOptions {
                    suffix: self.new_line.clone() + &self.new_line,
                    ..Default::default()
                };
            }
            return NodeOptions {
                suffix: self.new_line.clone(),
                ..Default::default()
            };
        } else if store.kind(before) == ast::Kind::VariableDeclaration {
            // insert `x = 1, ` into `const x = 1, y = 2;
            return NodeOptions {
                suffix: ", ".to_string(),
                ..Default::default()
            };
        } else if store.kind(before) == ast::Kind::Parameter {
            if inserted_store.kind(inserted) == ast::Kind::Parameter {
                return NodeOptions {
                    suffix: ", ".to_string(),
                    ..Default::default()
                };
            }
            return NodeOptions::default();
        } else if (store.kind(before) == ast::Kind::StringLiteral
            && store
                .parent(before)
                .is_some_and(|parent| store.kind(parent) == ast::Kind::ImportDeclaration))
            || store.kind(before) == ast::Kind::NamedImports
        {
            return NodeOptions {
                suffix: ", ".to_string(),
                ..Default::default()
            };
        } else if store.kind(before) == ast::Kind::ImportSpecifier {
            let mut suffix = ",".to_string();
            if blank_line_between {
                suffix.push_str(&self.new_line);
            } else {
                suffix.push(' ');
            }
            return NodeOptions {
                suffix,
                ..Default::default()
            };
        }
        // We haven't handled this kind of node yet -- add it
        panic!(
            "unimplemented node type {} in changeTracker.getOptionsForInsertNodeBefore",
            store.kind(before).to_string()
        );
    }

    pub fn get_insert_node_at_start_insert_options(
        &mut self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
        indentation: i32,
    ) -> NodeOptions {
        let node_key = node;
        let has_previous_insertion = self.nodes_with_insertions_at_start.contains_key(&node_key);
        self.nodes_with_insertions_at_start
            .entry(node_key)
            .or_insert(NodesInsertedAtStartState {
                node: node_key,
                source_file,
            });

        let members = get_members_or_properties(source_file.store(), node);
        let is_object_literal = ast::is_object_literal_expression(source_file.store(), node);
        let is_json = ast::is_json_source_file(source_file);

        let has_members = members.is_some_and(|members| !members.is_empty());

        let insert_trailing_comma = is_object_literal && (has_members || !is_json);
        let insert_leading_comma =
            is_object_literal && is_json && !has_members && has_previous_insertion;

        let mut suffix = String::new();
        if insert_trailing_comma {
            suffix = ",".to_string();
        } else if ast::is_interface_declaration(source_file.store(), node) && !has_members {
            suffix = ";".to_string();
        }

        let mut prefix = self.new_line.clone();
        if insert_leading_comma {
            prefix = ",".to_string() + &prefix;
        }

        NodeOptions {
            indentation: Some(indentation),
            prefix,
            suffix,
            ..Default::default()
        }
    }

    pub fn finish_nodes_with_insertions_at_start(&mut self) {
        for state in self
            .nodes_with_insertions_at_start
            .values()
            .copied()
            .collect::<Vec<_>>()
        {
            let store = state.source_file.store();
            let members = get_members_or_properties(store, state.node);
            let open_brace = find_child_token_or_scanned(
                state.node,
                ast::Kind::OpenBraceToken,
                state.source_file,
                members.as_ref().map(|members| members.pos()),
            );
            if open_brace.is_none() {
                continue;
            }

            let close_brace = find_child_token_or_scanned(
                state.node,
                ast::Kind::CloseBraceToken,
                state.source_file,
                Some(store.loc(state.node).end()),
            );
            if close_brace.is_none() {
                continue;
            }

            let open_brace = open_brace.unwrap();
            let close_brace = close_brace.unwrap();
            let is_empty = members.is_none() || members.unwrap().is_empty();
            let is_single_line = positions_are_on_same_line(
                open_brace.loc.end(),
                close_brace.loc.end(),
                state.source_file,
            );

            if is_empty && is_single_line && open_brace.loc.end() != close_brace.loc.end() - 1 {
                self.delete_range(
                    state.source_file,
                    core::new_text_range(open_brace.loc.end(), close_brace.loc.end() - 1),
                );
            }

            if is_single_line {
                let pos = self.converters.position_to_line_and_character(
                    state.source_file,
                    core::TextPos(close_brace.loc.end() - 1),
                );
                self.insert_text(state.source_file, pos, &self.new_line.clone());
            }
        }
    }
}

fn find_child_token_or_scanned(
    node: ast::Node,
    kind: ast::Kind,
    source_file: &ast::SourceFile,
    position: Option<i32>,
) -> Option<astnav::TokenInfo> {
    let store = source_file.store();
    astnav::find_child_of_kind_info(node, kind, source_file).or_else(|| {
        let token = astnav::find_preceding_token_info(source_file, position?)?;
        (token.kind == kind
            && token.loc.pos() >= store.loc(node).pos()
            && token.loc.end() <= store.loc(node).end())
        .then_some(token)
    })
}

pub fn get_members_or_properties(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::SourceNodeList<'_>> {
    if ast::is_object_literal_expression(store, node) {
        return store.properties(node);
    }
    store.members(node)
}

pub fn range_contains_range_exclusive(
    store: &ast::AstStore,
    outer: ast::Node,
    inner: ast::Node,
) -> bool {
    let outer = store.loc(outer);
    let inner = store.loc(inner);
    outer.pos() < inner.pos() && inner.end() < outer.end()
}

pub(crate) fn is_separator(
    store: &ast::AstStore,
    node: ast::Node,
    candidate: Option<ast::Node>,
) -> bool {
    candidate.is_some_and(|candidate| is_separator_kind(store, node, store.kind(candidate)))
}

fn is_separator_kind(store: &ast::AstStore, node: ast::Node, candidate: ast::Kind) -> bool {
    store.parent(node).is_some()
        && (candidate == ast::Kind::CommaToken
            || candidate == ast::Kind::SemicolonToken
                && store
                    .parent(node)
                    .is_some_and(|parent| store.kind(parent) == ast::Kind::ObjectLiteralExpression))
}

pub fn find_indentation_column(
    text: &str,
    line_start: i32,
    member_start: i32,
    tab_size: i32,
) -> i32 {
    let mut column = 0;

    for i in line_start..member_start.min(text.len() as i32) {
        let ch = text.as_bytes()[i as usize] as char;

        if stringutil::is_line_break(ch) {
            return -1;
        }
        if stringutil::is_white_space_single_line(ch) {
            column = advance_indentation_column(column, ch, tab_size);
            continue;
        }
        return column;
    }

    column
}

pub fn advance_indentation_column(column: i32, ch: char, tab_size: i32) -> i32 {
    if ch == '\t' {
        return column + tab_size - (column % tab_size);
    }
    column + 1
}
