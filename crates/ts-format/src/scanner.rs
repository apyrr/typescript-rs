use ts_ast as ast;
use ts_core as core;
use ts_scanner as scanner;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct TextRangeWithKind {
    pub loc: core::TextRange,
    pub kind: ast::Kind,
}

pub fn new_text_range_with_kind(pos: i32, end: i32, kind: ast::Kind) -> TextRangeWithKind {
    TextRangeWithKind {
        loc: core::new_text_range(pos, end),
        kind,
    }
}

#[derive(Clone, Default)]
pub struct TokenInfo {
    pub leading_trivia: Vec<TextRangeWithKind>,
    pub token: TextRangeWithKind,
    pub trailing_trivia: Vec<TextRangeWithKind>,
}

#[derive(Clone)]
pub struct FormattingScanner {
    pub s: scanner::Scanner,
    pub start_pos: i32,
    pub end_pos: i32,
    pub saved_pos: i32,
    pub has_last_token_info: bool,
    pub last_token_info: TokenInfo,
    pub last_scan_action: ScanAction,
    pub leading_trivia: Vec<TextRangeWithKind>,
    pub trailing_trivia: Vec<TextRangeWithKind>,
    pub was_new_line: bool,
}

pub fn new_formatting_scanner(
    text: String,
    language_variant: core::LanguageVariant,
    start_pos: i32,
    end_pos: i32,
    worker: &mut crate::span::FormatSpanWorker,
) -> Vec<core::TextChange> {
    let mut scan = scanner::new_scanner();
    scan.set_skip_trivia(false);
    scan.set_language_variant(language_variant);
    scan.set_text(text.into());
    scan.reset_token_state(start_pos);

    let fmt_scn = FormattingScanner {
        s: scan,
        start_pos,
        end_pos,
        saved_pos: 0,
        has_last_token_info: false,
        last_token_info: TokenInfo::default(),
        last_scan_action: ACTION_SCAN,
        leading_trivia: Vec::new(),
        trailing_trivia: Vec::new(),
        was_new_line: true,
    };

    worker.execute(fmt_scn)
}

impl FormattingScanner {
    pub fn advance(&mut self) {
        self.has_last_token_info = false;
        let is_started = self.s.token_full_start() != self.start_pos;

        if is_started {
            self.was_new_line = self
                .trailing_trivia
                .last()
                .is_some_and(|trivia| trivia.kind == ast::Kind::NewLineTrivia);
        } else {
            self.s.scan();
        }

        self.leading_trivia.clear();
        self.trailing_trivia.clear();

        let mut pos = self.s.token_full_start();

        // Read leading trivia and token
        while pos < self.end_pos {
            let t = self.s.token();
            if !ast::is_trivia(t) {
                break;
            }

            // consume leading trivia
            self.s.scan();
            let item = new_text_range_with_kind(pos, self.s.token_full_start(), t);

            pos = self.s.token_full_start();

            self.leading_trivia.push(item);
        }

        self.saved_pos = self.s.token_full_start();
    }
}

pub fn should_rescan_greater_than_token(store: &ast::AstStore, node: &ast::Node) -> bool {
    match store.kind(*node) {
        ast::Kind::GreaterThanEqualsToken
        | ast::Kind::GreaterThanGreaterThanEqualsToken
        | ast::Kind::GreaterThanGreaterThanGreaterThanEqualsToken
        | ast::Kind::GreaterThanGreaterThanGreaterThanToken
        | ast::Kind::GreaterThanGreaterThanToken => true,
        _ => false,
    }
}

pub fn should_rescan_jsx_identifier(store: &ast::AstStore, node: &ast::Node) -> bool {
    if let Some(parent) = store.parent(*node) {
        match store.kind(parent) {
            ast::Kind::JsxAttribute
            | ast::Kind::JsxOpeningElement
            | ast::Kind::JsxClosingElement
            | ast::Kind::JsxSelfClosingElement
            | ast::Kind::JsxNamespacedName => {
                // May parse an identifier like `module-layout`; that will be scanned as a keyword at first, but we should parse the whole thing to get an identifier.
                let node_kind = store.kind(*node);
                return ast::is_keyword_kind(node_kind) || node_kind == ast::Kind::Identifier;
            }
            _ => {}
        }
    }
    false
}

impl FormattingScanner {
    pub fn should_rescan_jsx_text(&self, store: &ast::AstStore, node: &ast::Node) -> bool {
        if ast::is_jsx_text(store, *node) {
            return true;
        }
        if !ast::is_jsx_element(store, *node) || self.has_last_token_info == false {
            return false;
        }

        self.last_token_info.token.kind == ast::Kind::JsxText
    }
}

pub fn should_rescan_slash_token(store: &ast::AstStore, container: &ast::Node) -> bool {
    store.kind(*container) == ast::Kind::RegularExpressionLiteral
}

pub fn should_rescan_template_token(store: &ast::AstStore, container: &ast::Node) -> bool {
    let kind = store.kind(*container);
    kind == ast::Kind::TemplateMiddle || kind == ast::Kind::TemplateTail
}

pub fn should_rescan_jsx_attribute_value(store: &ast::AstStore, node: &ast::Node) -> bool {
    store.parent(*node).is_some_and(|parent| {
        ast::is_jsx_attribute(store, parent) && store.initializer(parent).as_ref() == Some(node)
    })
}

pub fn starts_with_slash_token(t: ast::Kind) -> bool {
    t == ast::Kind::SlashToken || t == ast::Kind::SlashEqualsToken
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScanAction(pub i32);

pub const ACTION_SCAN: ScanAction = ScanAction(0);
pub const ACTION_RESCAN_GREATER_THAN_TOKEN: ScanAction = ScanAction(1);
pub const ACTION_RESCAN_SLASH_TOKEN: ScanAction = ScanAction(2);
pub const ACTION_RESCAN_TEMPLATE_TOKEN: ScanAction = ScanAction(3);
pub const ACTION_RESCAN_JSX_IDENTIFIER: ScanAction = ScanAction(4);
pub const ACTION_RESCAN_JSX_TEXT: ScanAction = ScanAction(5);
pub const ACTION_RESCAN_JSX_ATTRIBUTE_VALUE: ScanAction = ScanAction(6);

pub fn fix_token_kind(
    store: &ast::AstStore,
    mut token_info: TokenInfo,
    container: &ast::Node,
) -> TokenInfo {
    let container_kind = store.kind(*container);
    if ast::is_token_kind(container_kind) && token_info.token.kind != container_kind {
        token_info.token.kind = container_kind;
    }
    token_info
}

impl FormattingScanner {
    pub fn read_token_info(&mut self, store: &ast::AstStore, n: &ast::Node) -> TokenInfo {
        debug_assert!(self.is_on_token());

        // normally scanner returns the smallest available token
        // check the kind of context node to determine if scanner should have more greedy behavior and consume more text.

        let expected_scan_action;
        if should_rescan_greater_than_token(store, n) {
            expected_scan_action = ACTION_RESCAN_GREATER_THAN_TOKEN;
        } else if should_rescan_slash_token(store, n) {
            expected_scan_action = ACTION_RESCAN_SLASH_TOKEN;
        } else if should_rescan_template_token(store, n) {
            expected_scan_action = ACTION_RESCAN_TEMPLATE_TOKEN;
        } else if should_rescan_jsx_identifier(store, n) {
            expected_scan_action = ACTION_RESCAN_JSX_IDENTIFIER;
        } else if self.should_rescan_jsx_text(store, n) {
            expected_scan_action = ACTION_RESCAN_JSX_TEXT;
        } else if should_rescan_jsx_attribute_value(store, n) {
            expected_scan_action = ACTION_RESCAN_JSX_ATTRIBUTE_VALUE;
        } else {
            expected_scan_action = ACTION_SCAN;
        }

        if self.has_last_token_info && expected_scan_action == self.last_scan_action {
            // readTokenInfo was called before with the same expected scan action.
            // No need to re-scan text, return existing 'lastTokenInfo'
            // it is ok to call fixTokenKind here since it does not affect
            // what portion of text is consumed. In contrast rescanning can change it,
            // i.e. for '>=' when originally scanner eats just one character
            // and rescanning forces it to consume more.
            self.last_token_info = fix_token_kind(store, self.last_token_info.clone(), n);
            return self.last_token_info.clone();
        }

        if self.s.token_full_start() != self.saved_pos {
            // readTokenInfo was called before but scan action differs - rescan text
            self.s.reset_token_state(self.saved_pos);
            self.s.scan();
        }

        let mut current_token = self.get_next_token(store, n, expected_scan_action);

        let token =
            new_text_range_with_kind(self.s.token_full_start(), self.s.token_end(), current_token);

        // consume trailing trivia
        self.trailing_trivia.clear();
        while self.s.token_full_start() < self.end_pos {
            current_token = self.s.scan();
            if !ast::is_trivia(current_token) {
                break;
            }
            let trivia = new_text_range_with_kind(
                self.s.token_full_start(),
                self.s.token_end(),
                current_token,
            );

            self.trailing_trivia.push(trivia);

            if current_token == ast::Kind::NewLineTrivia {
                // move past new line
                self.s.scan();
                break;
            }
        }

        self.has_last_token_info = true;
        self.last_token_info = TokenInfo {
            leading_trivia: self.leading_trivia.clone(),
            token,
            trailing_trivia: self.trailing_trivia.clone(),
        };
        self.last_token_info = fix_token_kind(store, self.last_token_info.clone(), n);

        self.last_token_info.clone()
    }

    pub fn get_next_token(
        &mut self,
        store: &ast::AstStore,
        n: &ast::Node,
        expected_scan_action: ScanAction,
    ) -> ast::Kind {
        let token = self.s.token();
        self.last_scan_action = ACTION_SCAN;
        match expected_scan_action {
            ACTION_RESCAN_GREATER_THAN_TOKEN => {
                if token == ast::Kind::GreaterThanToken {
                    self.last_scan_action = ACTION_RESCAN_GREATER_THAN_TOKEN;
                    let new_token = self.s.re_scan_greater_than_token();
                    debug_assert!(store.kind(*n) == new_token);
                    return new_token;
                }
            }
            ACTION_RESCAN_SLASH_TOKEN => {
                if starts_with_slash_token(token) {
                    self.last_scan_action = ACTION_RESCAN_SLASH_TOKEN;
                    let new_token = self.s.re_scan_slash_token();
                    debug_assert!(store.kind(*n) == new_token);
                    return new_token;
                }
            }
            ACTION_RESCAN_TEMPLATE_TOKEN => {
                if token == ast::Kind::CloseBraceToken {
                    self.last_scan_action = ACTION_RESCAN_TEMPLATE_TOKEN;
                    return self.s.re_scan_template_token(false /*isTaggedTemplate*/);
                }
            }
            ACTION_RESCAN_JSX_IDENTIFIER => {
                self.last_scan_action = ACTION_RESCAN_JSX_IDENTIFIER;
                return self.s.scan_jsx_identifier();
            }
            ACTION_RESCAN_JSX_TEXT => {
                self.last_scan_action = ACTION_RESCAN_JSX_TEXT;
                return self.s.re_scan_jsx_token(false /*allowMultilineJsxText*/);
            }
            ACTION_RESCAN_JSX_ATTRIBUTE_VALUE => {
                self.last_scan_action = ACTION_RESCAN_JSX_ATTRIBUTE_VALUE;
                return self.s.re_scan_jsx_attribute_value();
            }
            ACTION_SCAN => {}
            _ => unreachable!("unhandled scan action kind"),
        }
        token
    }

    pub fn read_eof_token_range(&self) -> TextRangeWithKind {
        debug_assert!(self.is_on_eof());
        new_text_range_with_kind(
            self.s.token_full_start(),
            self.s.token_end(),
            ast::Kind::EndOfFile,
        )
    }

    pub fn is_on_token(&self) -> bool {
        let mut current = self.s.token();
        if self.has_last_token_info {
            current = self.last_token_info.token.kind;
        }
        current != ast::Kind::EndOfFile && !ast::is_trivia(current)
    }

    pub fn is_on_eof(&self) -> bool {
        let mut current = self.s.token();
        if self.has_last_token_info {
            current = self.last_token_info.token.kind;
        }
        current == ast::Kind::EndOfFile
    }

    pub fn skip_to_end_of(&mut self, r: &core::TextRange) {
        self.s.reset_token_state(r.end());
        self.saved_pos = self.s.token_full_start();
        self.last_scan_action = ACTION_SCAN;
        self.has_last_token_info = false;
        self.was_new_line = false;
        self.leading_trivia.clear();
        self.trailing_trivia.clear();
    }

    pub fn skip_to_start_of(&mut self, r: &core::TextRange) {
        self.s.reset_token_state(r.pos());
        self.saved_pos = self.s.token_full_start();
        self.last_scan_action = ACTION_SCAN;
        self.has_last_token_info = false;
        self.was_new_line = false;
        self.leading_trivia.clear();
        self.trailing_trivia.clear();
    }

    pub fn get_current_leading_trivia(&self) -> Vec<TextRangeWithKind> {
        self.leading_trivia.clone()
    }

    pub fn last_trailing_trivia_was_new_line(&self) -> bool {
        self.was_new_line
    }

    pub fn get_token_full_start(&self) -> i32 {
        if self.has_last_token_info {
            return self.last_token_info.token.loc.pos();
        }
        self.s.token_full_start()
    }

    pub fn get_start_pos(&self) -> i32 {
        self.get_token_full_start()
    }
}
