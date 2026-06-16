use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_scanner as scanner;

use crate::FormatRequestKind;
use crate::scanner::TextRangeWithKind;
use crate::util::range_is_on_one_line;

pub struct FormattingContext {
    pub current_token_span: TextRangeWithKind,
    pub next_token_span: TextRangeWithKind,
    pub context_node: Option<ast::Node>,
    pub current_token_parent: Option<ast::Node>,
    pub next_token_parent: Option<ast::Node>,

    pub context_node_all_on_same_line: core::Tristate,
    pub next_node_all_on_same_line: core::Tristate,
    pub tokens_are_on_same_line: core::Tristate,
    pub context_node_block_is_on_one_line: core::Tristate,
    pub next_node_block_is_on_one_line: core::Tristate,

    pub source_file: ast::SourceFile,
    pub formatting_request_kind: FormatRequestKind,
    pub options: crate::lsutil::FormatCodeSettings,
}

pub fn new_formatting_context(
    file: ast::SourceFile,
    kind: FormatRequestKind,
    options: crate::lsutil::FormatCodeSettings,
) -> FormattingContext {
    FormattingContext {
        source_file: file,
        formatting_request_kind: kind,
        options,
        current_token_span: TextRangeWithKind::default(),
        next_token_span: TextRangeWithKind::default(),
        context_node: None,
        current_token_parent: None,
        next_token_parent: None,
        context_node_all_on_same_line: core::TS_UNKNOWN,
        next_node_all_on_same_line: core::TS_UNKNOWN,
        tokens_are_on_same_line: core::TS_UNKNOWN,
        context_node_block_is_on_one_line: core::TS_UNKNOWN,
        next_node_block_is_on_one_line: core::TS_UNKNOWN,
    }
}

impl FormattingContext {
    pub fn update_context(
        &mut self,
        cur: TextRangeWithKind,
        cur_parent: Option<ast::Node>,
        next: TextRangeWithKind,
        next_parent: Option<ast::Node>,
        common_parent: Option<ast::Node>,
    ) {
        if cur_parent.is_none() {
            panic!("nil current range node parent in update context");
        }
        if next_parent.is_none() {
            panic!("nil next range node parent in update context");
        }
        if common_parent.is_none() {
            panic!("nil common parent node in update context");
        }
        self.current_token_span = cur;
        self.current_token_parent = cur_parent;
        self.next_token_span = next;
        self.next_token_parent = next_parent;
        self.context_node = common_parent;

        // drop cached results
        self.context_node_all_on_same_line = core::TS_UNKNOWN;
        self.next_node_all_on_same_line = core::TS_UNKNOWN;
        self.tokens_are_on_same_line = core::TS_UNKNOWN;
        self.context_node_block_is_on_one_line = core::TS_UNKNOWN;
        self.next_node_block_is_on_one_line = core::TS_UNKNOWN;
    }

    pub fn range_is_on_one_line(&self, node: core::TextRange) -> core::Tristate {
        if range_is_on_one_line(node, &self.source_file) {
            return core::TS_TRUE;
        }
        core::TS_FALSE
    }

    pub fn node_is_on_one_line(&self, node: &ast::Node) -> core::Tristate {
        self.range_is_on_one_line(with_token_start(node, &self.source_file))
    }
}

pub fn with_token_start(loc: &ast::Node, file: &ast::SourceFile) -> core::TextRange {
    let start_pos = scanner::get_token_pos_of_node(loc, file, false);
    core::new_text_range(start_pos as i32, file.store().loc(*loc).end())
}

impl FormattingContext {
    pub fn block_is_on_one_line(&self, node: &ast::Node) -> core::Tristate {
        let open_brace =
            astnav::find_child_of_kind(*node, ast::Kind::OpenBraceToken, &self.source_file);
        let close_brace =
            astnav::find_child_of_kind(*node, ast::Kind::CloseBraceToken, &self.source_file);
        if let (Some(open_brace), Some(close_brace)) = (open_brace, close_brace) {
            let close_brace_start =
                scanner::get_token_pos_of_node(&close_brace, &self.source_file, false);
            return self.range_is_on_one_line(core::new_text_range(
                self.source_file.store().loc(open_brace).end(),
                close_brace_start as i32,
            ));
        }
        core::TS_FALSE
    }

    pub fn context_node_all_on_same_line(&mut self) -> bool {
        if self.context_node_all_on_same_line == core::TS_UNKNOWN {
            self.context_node_all_on_same_line =
                self.node_is_on_one_line(self.context_node.as_ref().unwrap());
        }
        self.context_node_all_on_same_line == core::TS_TRUE
    }

    pub fn next_node_all_on_same_line(&mut self) -> bool {
        if self.next_node_all_on_same_line == core::TS_UNKNOWN {
            self.next_node_all_on_same_line =
                self.node_is_on_one_line(self.next_token_parent.as_ref().unwrap());
        }
        self.next_node_all_on_same_line == core::TS_TRUE
    }

    pub fn tokens_are_on_same_line(&mut self) -> bool {
        if self.tokens_are_on_same_line == core::TS_UNKNOWN {
            self.tokens_are_on_same_line = self.range_is_on_one_line(core::new_text_range(
                self.current_token_span.loc.pos(),
                self.next_token_span.loc.end(),
            ));
        }
        self.tokens_are_on_same_line == core::TS_TRUE
    }

    pub fn context_node_block_is_on_one_line(&mut self) -> bool {
        if self.context_node_block_is_on_one_line == core::TS_UNKNOWN {
            self.context_node_block_is_on_one_line =
                self.block_is_on_one_line(self.context_node.as_ref().unwrap());
        }
        self.context_node_block_is_on_one_line == core::TS_TRUE
    }

    pub fn next_node_block_is_on_one_line(&mut self) -> bool {
        if self.next_node_block_is_on_one_line == core::TS_UNKNOWN {
            self.next_node_block_is_on_one_line =
                self.block_is_on_one_line(self.next_token_parent.as_ref().unwrap());
        }
        self.next_node_block_is_on_one_line == core::TS_TRUE
    }
}
