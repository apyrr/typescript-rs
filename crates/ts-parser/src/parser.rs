use std::{cell::RefCell, sync::Arc};

use smallvec::SmallVec;
use ts_ast as ast;
use ts_collections::{self as collections, FastHashMap, FastHashMapExt};
use ts_core as core;
use ts_debug as debug;
use ts_diagnostics as diagnostics;
use ts_scanner as scanner;
use ts_tspath as tspath;

use crate::references::collect_external_module_references;
use crate::utilities::get_language_variant;

macro_rules! finish_node {
    ($parser:expr, $node:expr, $pos:expr) => {{
        // PORT NOTE: reshaped for borrowck
        let node = $node;
        let pos = $pos;
        $parser.finish_node(node, pos)
    }};
}

fn required_tag_name(store: &ast::AstStore, node: &ast::Node) -> ast::Node {
    store.tag_name(*node).expect("JSX tag_name")
}

fn child_exists(_child: &ast::Node) -> bool {
    true
}

fn required_jsx_opening_element(store: &ast::AstStore, node: &ast::Node) -> ast::Node {
    store.jsx_opening_element(*node)
}

fn required_jsx_closing_element(store: &ast::AstStore, node: &ast::Node) -> ast::Node {
    store.jsx_closing_element(*node)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ParsingContext {
    // Elements in source file
    PCSourceElements = 0,
    // Statements in block
    PCBlockStatements,
    // Clauses in switch statement
    PCSwitchClauses,
    // Statements in switch clause
    PCSwitchClauseStatements,
    // Members in interface or type literal
    PCTypeMembers,
    // Members in class declaration
    PCClassMembers,
    // Members in enum declaration
    PCEnumMembers,
    // Elements in a heritage clause
    PCHeritageClauseElement,
    // Variable declarations in variable statement
    PCVariableDeclarations,
    // Binding elements in object binding list
    PCObjectBindingElements,
    // Binding elements in array binding list
    PCArrayBindingElements,
    // Expressions in argument list
    PCArgumentExpressions,
    // Members in object literal
    PCObjectLiteralMembers,
    // Attributes in jsx element
    PCJsxAttributes,
    // Things between opening and closing JSX tags
    PCJsxChildren,
    // Members in array literal
    PCArrayLiteralMembers,
    // Parameters in parameter list
    PCParameters,
    // Property names in a rest type list
    PCRestProperties,
    // Type parameters in type parameter list
    PCTypeParameters,
    // Type arguments in type argument list
    PCTypeArguments,
    // Element types in tuple element type list
    PCTupleElementTypes,
    // Heritage clauses for a class or interface declaration.
    PCHeritageClauses,
    // Named import clause's import specifier list
    PCImportOrExportSpecifiers,
    PCImportAttributes,
    // Number of parsing contexts
    PCCount,
}

pub type ParsingContexts = i32;

pub struct Parser {
    pub scanner: Option<scanner::Scanner>,
    pub factory: ast::NodeFactory,
    pub opts: ast::SourceFileParseOptions,
    pub source_text: Arc<str>,
    pub script_kind: core::ScriptKind,
    pub language_variant: core::LanguageVariant,
    pub diagnostics: Vec<ast::Diagnostic>,
    pub js_diagnostics: Vec<ast::Diagnostic>,
    pub token: ast::Kind,
    pub source_flags: ast::NodeFlags,
    pub context_flags: ast::NodeFlags,
    pub parsing_contexts: ParsingContexts,
    pub statement_has_await_identifier: bool,
    pub has_deprecated_tag: bool,
    pub has_parse_error: bool,
    pub identifiers: FastHashMap<String, String>,
    pub identifier_count: i32,
    pub not_parenthesized_arrow: collections::Set<i32>,
    pub node_slice_arena: core::Arena<ast::Node>,
    pub string_slice_arena: core::Arena<String>,
    pub possible_await_spans: Vec<i32>,
    pub reparse_list: Vec<ast::Node>,
    pub common_js_module_indicator: Option<ast::Node>,
    pub current_parent: Option<ast::Node>,
    pub reparsed_clones: Vec<ast::Node>,
    pub source_hash: u128,
}

pub fn new_parser() -> Parser {
    Parser {
        scanner: None,
        factory: ast::NodeFactory::default(),
        opts: ast::SourceFileParseOptions::default(),
        source_text: Arc::<str>::from(""),
        script_kind: core::ScriptKind::Unknown,
        language_variant: core::LanguageVariant::Standard,
        diagnostics: Vec::new(),
        js_diagnostics: Vec::new(),
        token: ast::Kind::Unknown,
        source_flags: ast::NodeFlags::NONE,
        context_flags: ast::NodeFlags::NONE,
        parsing_contexts: 0,
        statement_has_await_identifier: false,
        has_deprecated_tag: false,
        has_parse_error: false,
        identifiers: FastHashMap::new(),
        identifier_count: 0,
        not_parenthesized_arrow: collections::Set::default(),
        node_slice_arena: core::Arena::default(),
        string_slice_arena: core::Arena::default(),
        possible_await_spans: Vec::new(),
        reparse_list: Vec::new(),
        common_js_module_indicator: None,
        current_parent: None,
        reparsed_clones: Vec::new(),
        source_hash: 0,
    }
}

pub fn viable_keyword_suggestions() -> Vec<String> {
    scanner::get_viable_keyword_suggestions()
}

pub fn get_space_suggestion(expression_text: &str) -> String {
    for keyword in viable_keyword_suggestions() {
        if expression_text.len() > keyword.len() + 2 && expression_text.starts_with(&keyword) {
            return format!("{} {}", keyword, &expression_text[keyword.len()..]);
        }
    }
    String::new()
}

thread_local! {
    static PARSER_POOL: RefCell<Vec<Parser>> = RefCell::new(Vec::new());
}

const PARSER_POOL_MAX: usize = 8;

fn get_parser() -> Parser {
    PARSER_POOL
        .with(|pool| pool.borrow_mut().pop())
        .unwrap_or_else(new_parser)
}

fn put_parser(mut p: Parser) {
    let scanner = p.scanner.take().map(|mut scanner| {
        scanner.reset();
        scanner
    });
    p = new_parser();
    p.scanner = scanner;
    PARSER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < PARSER_POOL_MAX {
            pool.push(p);
        }
    });
}

fn estimated_node_capacity(source_len: usize) -> usize {
    (source_len / 24 + 128).min(65_536)
}

pub fn parse_source_file(
    opts: ast::SourceFileParseOptions,
    source_text: impl Into<Arc<str>>,
    script_kind: core::ScriptKind,
) -> ast::SourceFile {
    parse_source_file_with_hash(opts, source_text, script_kind, 0)
}

pub fn parse_source_file_with_hash(
    opts: ast::SourceFileParseOptions,
    source_text: impl Into<Arc<str>>,
    script_kind: core::ScriptKind,
    source_hash: u128,
) -> ast::SourceFile {
    parse_source_file_as_parsed_with_hash(opts, source_text, script_kind, source_hash)
        .into_source_file()
}

pub fn parse_source_file_as_parsed(
    opts: ast::SourceFileParseOptions,
    source_text: impl Into<Arc<str>>,
    script_kind: core::ScriptKind,
) -> ast::ParsedSourceFile {
    parse_source_file_as_parsed_with_hash(opts, source_text, script_kind, 0)
}

pub fn parse_source_file_as_parsed_with_hash(
    opts: ast::SourceFileParseOptions,
    source_text: impl Into<Arc<str>>,
    script_kind: core::ScriptKind,
    source_hash: u128,
) -> ast::ParsedSourceFile {
    let mut p = get_parser();
    let source_text = source_text.into();
    p.initialize_state(opts, source_text, script_kind);
    p.source_hash = source_hash;
    p.next_token();
    let result = if p.script_kind == core::ScriptKind::JSON {
        p.parse_json_text()
    } else {
        p.parse_source_file_worker()
    };
    put_parser(p);
    result
}

impl Parser {
    pub fn is_javascript(&self) -> bool {
        matches!(
            self.script_kind,
            core::ScriptKind::JS | core::ScriptKind::JSX
        )
    }

    pub fn parse_json_text(&mut self) -> ast::ParsedSourceFile {
        let pos = self.node_pos();
        let (statements, eof) = if self.token == ast::Kind::EndOfFile {
            (
                self.new_parser_node_list(core::TextRange::new(pos, self.node_pos()), Vec::new()),
                self.parse_token_node(),
            )
        } else {
            let mut expressions: Vec<ast::Node> = Vec::new();
            while self.token != ast::Kind::EndOfFile {
                let expression = match self.token {
                    ast::Kind::OpenBracketToken => self.parse_array_literal_expression(),
                    ast::Kind::TrueKeyword | ast::Kind::FalseKeyword | ast::Kind::NullKeyword => {
                        self.parse_token_node()
                    }
                    ast::Kind::MinusToken => {
                        if self.look_ahead(|p| {
                            p.next_token() == ast::Kind::NumericLiteral
                                && p.next_token() != ast::Kind::ColonToken
                        }) {
                            self.parse_prefix_unary_expression()
                        } else {
                            self.parse_object_literal_expression()
                        }
                    }
                    ast::Kind::NumericLiteral | ast::Kind::StringLiteral => {
                        if self.look_ahead(|p| p.next_token() != ast::Kind::ColonToken) {
                            self.parse_literal_expression(false)
                        } else {
                            self.parse_object_literal_expression()
                        }
                    }
                    _ => self.parse_object_literal_expression(),
                };

                if expressions.is_empty() && self.token != ast::Kind::EndOfFile {
                    self.parse_error_at_current_token(&diagnostics::UNEXPECTED_TOKEN, Vec::new());
                }
                expressions.push(expression);
            }

            let expression = if expressions.len() > 1 {
                let elements = self
                    .new_parser_node_list(core::TextRange::new(pos, self.node_pos()), expressions);
                // PORT NOTE: reshaped for borrowck
                let node = self.factory.new_array_literal_expression(elements, false);
                self.finish_node(node, pos)
            } else {
                expressions.remove(0)
            };
            // PORT NOTE: reshaped for borrowck
            let statement = self.factory.new_expression_statement(expression);
            let statement = self.finish_node(statement, pos);
            (
                self.new_parser_node_list(
                    core::TextRange::new(pos, self.node_pos()),
                    vec![statement],
                ),
                self.parse_expected_token(ast::Kind::EndOfFile),
            )
        };
        // PORT NOTE: reshaped for borrowck
        let node = self.factory.new_source_file(
            self.opts.clone(),
            Arc::clone(&self.source_text),
            statements,
            eof,
        );
        let node = self.finish_node(node, pos);
        if let Some(statement) = self.factory.parsed_node_list_first(statements) {
            let expression = self.factory.store().expression(statement);
            let mut diagnostics = Vec::new();
            validate_json_value(
                self.factory.store(),
                &self.opts.file_name,
                &self.source_text,
                expression,
                &mut diagnostics,
            );
            self.diagnostics.extend(diagnostics);
        }
        let metadata = self.parsed_source_file_metadata(node, false, false);
        std::mem::take(&mut self.factory).finish_parsed_source_file_as_parsed(node, metadata)
    }

    pub fn initialize_state(
        &mut self,
        opts: ast::SourceFileParseOptions,
        source_text: Arc<str>,
        script_kind: core::ScriptKind,
    ) {
        if script_kind == core::ScriptKind::Unknown {
            panic!(
                "ScriptKind must be specified when parsing source file: {}",
                opts.file_name
            );
        }
        if self.scanner.is_none() {
            self.scanner = Some(scanner::new_scanner());
        } else if let Some(scanner) = &mut self.scanner {
            scanner.reset();
        }
        self.opts = opts;
        self.source_text = source_text;
        self.factory = self
            .factory
            .fresh_with_arena_capacity(estimated_node_capacity(self.source_text.len()));
        self.script_kind = script_kind;
        self.language_variant = get_language_variant(self.script_kind);
        self.context_flags = match self.script_kind {
            core::ScriptKind::JS | core::ScriptKind::JSX => ast::NodeFlags::JAVA_SCRIPT_FILE,
            core::ScriptKind::JSON => ast::NodeFlags::JAVA_SCRIPT_FILE | ast::NodeFlags::JSON_FILE,
            _ => ast::NodeFlags::NONE,
        };
        if let Some(scanner) = &mut self.scanner {
            scanner.set_text(Arc::clone(&self.source_text));
            scanner.set_on_error(None);
            scanner.set_language_variant(self.language_variant);
        }
    }

    pub fn scan_error(
        &mut self,
        message: &diagnostics::Message,
        pos: i32,
        length: i32,
        args: Vec<diagnostics::Any>,
    ) {
        self.parse_error_at_range(core::TextRange::new(pos, pos + length), message, args);
    }

    pub fn parse_error_at(
        &mut self,
        pos: i32,
        end: i32,
        message: &diagnostics::Message,
        args: Vec<diagnostics::Any>,
    ) -> ast::Diagnostic {
        self.parse_error_at_range(core::TextRange::new(pos, end), message, args)
    }

    pub fn parse_error_at_current_token(
        &mut self,
        message: &diagnostics::Message,
        args: Vec<diagnostics::Any>,
    ) -> ast::Diagnostic {
        let range = self.scanner.as_ref().expect("scanner").token_range();
        self.parse_error_at_range(range, message, args)
    }

    pub fn parse_error_at_range(
        &mut self,
        loc: core::TextRange,
        message: &diagnostics::Message,
        args: Vec<diagnostics::Any>,
    ) -> ast::Diagnostic {
        let result = ast::new_diagnostic(None, loc, message, &args);
        if self
            .diagnostics
            .last()
            .map(|d| d.pos() != loc.pos())
            .unwrap_or(true)
        {
            self.diagnostics.push(result.clone());
        }
        self.has_parse_error = true;
        result
    }

    pub fn next_token(&mut self) -> ast::Kind {
        if ast::is_keyword(self.token)
            && (self.scanner.as_ref().expect("scanner").has_unicode_escape()
                || self
                    .scanner
                    .as_ref()
                    .expect("scanner")
                    .has_extended_unicode_escape())
        {
            self.parse_error_at_current_token(
                &diagnostics::KEYWORDS_CANNOT_CONTAIN_ESCAPE_CHARACTERS,
                Vec::new(),
            );
        }
        self.next_token_without_check()
    }

    pub fn next_token_without_check(&mut self) -> ast::Kind {
        let token = self.scanner.as_mut().expect("scanner").scan();
        self.drain_scanner_diagnostics();
        self.token = token;
        token
    }

    pub fn drain_scanner_diagnostics(&mut self) {
        let scanner = self.scanner.as_mut().expect("scanner");
        if scanner.diagnostics.is_empty() {
            return;
        }
        let diagnostics = std::mem::take(&mut scanner.diagnostics);
        for diagnostic in diagnostics {
            let args = diagnostic
                .args
                .into_iter()
                .map(diagnostics::Any::from)
                .collect();
            self.parse_error_at_range(
                core::TextRange::new(
                    diagnostic.start as i32,
                    (diagnostic.start + diagnostic.length) as i32,
                ),
                &diagnostic.message,
                args,
            );
        }
    }

    pub fn node_pos(&self) -> i32 {
        self.scanner.as_ref().expect("scanner").token_full_start()
    }

    pub fn parse_source_file_worker(&mut self) -> ast::ParsedSourceFile {
        let is_declaration_file = tspath::is_declaration_file_name(&self.opts.file_name);
        if is_declaration_file {
            self.context_flags |= ast::NodeFlags::AMBIENT;
        }
        let pos = self.node_pos();
        let mut statements = self.parse_list_index(
            ParsingContext::PCSourceElements,
            Parser::parse_toplevel_statement,
        );
        let end = self.node_pos();
        let eof = self.parse_token_node();
        if self.factory.store().kind(eof) != ast::Kind::EndOfFile {
            panic!("Expected end of file token from scanner.");
        }
        if !self.reparse_list.is_empty() {
            statements.extend(self.reparse_list.drain(..));
        }
        // PORT NOTE: reshaped for borrowck
        let statement_list = self.new_parser_node_list(core::TextRange::new(pos, end), statements);
        let node = self.factory.new_source_file(
            self.opts.clone(),
            Arc::clone(&self.source_text),
            statement_list,
            eof,
        );
        let node = self.finish_node(node, pos);
        let mut root = node;
        let may_reparse_top_level_await =
            !is_declaration_file && !self.possible_await_spans.is_empty();
        let mut metadata = self.parsed_source_file_metadata(
            root,
            is_declaration_file,
            may_reparse_top_level_await,
        );
        if !metadata.is_declaration_file
            && metadata.external_module_indicator.is_some()
            && !self.possible_await_spans.is_empty()
        {
            // PORT NOTE: reshaped for borrowck
            let reparse = self.reparse_top_level_await(root);
            let reparse = self.finish_node(reparse, pos);
            root = reparse;
            metadata = self.parsed_source_file_metadata(root, is_declaration_file, false);
        }
        std::mem::take(&mut self.factory).finish_parsed_source_file_as_parsed(root, metadata)
    }

    fn parsed_source_file_metadata(
        &mut self,
        root: ast::Node,
        is_declaration_file: bool,
        retain_identifiers: bool,
    ) -> ast::ParsedSourceFileMetadata {
        let pragmas = get_comment_pragmas(&mut self.factory, &self.source_text);
        let (
            referenced_files,
            type_reference_directives,
            lib_reference_directives,
            check_js_directive,
        ) = self.reference_directives_from_pragmas(&pragmas);
        let diagnostics = self.diagnostics.clone();
        let js_diagnostics = if self.is_javascript() {
            self.js_diagnostics.clone()
        } else {
            Vec::new()
        };
        let mut reparsed_clones = self.reparsed_clones.clone();
        reparsed_clones.sort_by_key(|node| {
            let loc = self.factory.store().loc(*node);
            (loc.pos(), loc.end())
        });
        let external_module_indicator = ast::get_external_module_indicator(
            self.factory.store(),
            root,
            self.source_flags,
            self.script_kind,
            is_declaration_file,
            self.opts.external_module_indicator_options,
        );
        let references = collect_external_module_references(
            self.factory.store(),
            root,
            self.source_flags,
            is_declaration_file,
            external_module_indicator,
        );
        let identifiers = if retain_identifiers {
            Arc::new(self.identifiers.clone())
        } else {
            Arc::new(std::mem::take(&mut self.identifiers))
        };
        ast::ParsedSourceFileMetadata {
            diagnostics,
            js_diagnostics,
            comment_directives: self
                .scanner
                .as_ref()
                .expect("scanner")
                .comment_directives()
                .to_vec(),
            pragmas,
            referenced_files,
            type_reference_directives,
            lib_reference_directives,
            check_js_directive,
            common_js_module_indicator: self.common_js_module_indicator.clone(),
            is_declaration_file,
            contains_non_ascii: self.scanner.as_ref().expect("scanner").contains_non_ascii(),
            language_variant: self.language_variant,
            script_kind: self.script_kind,
            source_flags: self.source_flags,
            identifiers,
            node_count: self.factory.node_count(),
            text_count: self.factory.text_count(),
            identifier_count: self.identifier_count,
            reparsed_clones,
            imports: references.imports,
            module_augmentations: references.module_augmentations,
            ambient_module_names: references.ambient_module_names,
            uses_uri_style_node_core_modules: references.uses_uri_style_node_core_modules,
            external_module_indicator,
            hash: self.source_hash,
        }
    }

    pub fn parse_token_node(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let kind = self.token;
        self.next_token();
        // PORT NOTE: reshaped for borrowck
        let node = self.factory.new_token(kind);
        self.finish_node(node, pos)
    }

    pub fn parse_expected_token(&mut self, kind: ast::Kind) -> ast::Node {
        if let Some(token) = self.parse_optional_token(kind) {
            return token;
        }
        self.parse_error_at_current_token(
            &diagnostics::X_0_EXPECTED,
            vec![diagnostics::Any::from(scanner::token_to_string(kind))],
        );
        // PORT NOTE: reshaped for borrowck
        let pos = self.node_pos();
        let node = self.factory.new_token(kind);
        self.finish_node(node, pos)
    }

    pub fn parse_object_literal_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let open_brace_position = self.scanner.as_ref().expect("scanner").token_start();
        let open_brace_parsed = self.parse_expected(ast::Kind::OpenBraceToken);
        let multi_line = self.has_preceding_line_break();
        let properties = self.parse_delimited_list(
            ParsingContext::PCObjectLiteralMembers,
            Parser::parse_object_literal_element,
        );
        self.parse_expected_matching_brackets(
            ast::Kind::OpenBraceToken,
            ast::Kind::CloseBraceToken,
            open_brace_parsed,
            open_brace_position,
        );
        // PORT NOTE: reshaped for borrowck
        let node = self
            .factory
            .new_object_literal_expression(properties, multi_line);
        self.finish_node(node, pos)
    }

    pub fn parse_array_literal_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let open_bracket_position = self.scanner.as_ref().expect("scanner").token_start();
        let open_bracket_parsed = self.parse_expected(ast::Kind::OpenBracketToken);
        let multi_line = self.has_preceding_line_break();
        let elements = self.parse_delimited_list(
            ParsingContext::PCArrayLiteralMembers,
            Parser::parse_argument_or_array_literal_element,
        );
        self.parse_expected_matching_brackets(
            ast::Kind::OpenBracketToken,
            ast::Kind::CloseBracketToken,
            open_bracket_parsed,
            open_bracket_position,
        );
        // PORT NOTE: reshaped for borrowck
        let node = self
            .factory
            .new_array_literal_expression(elements, multi_line);
        self.finish_node(node, pos)
    }

    pub fn parse_argument_or_array_literal_element(&mut self) -> ast::Node {
        match self.token {
            ast::Kind::DotDotDotToken => self.parse_spread_element(),
            ast::Kind::CommaToken => {
                // PORT NOTE: reshaped for borrowck
                let pos = self.node_pos();
                let node = self.factory.new_omitted_expression();
                self.finish_node(node, pos)
            }
            _ => self.parse_assignment_expression_or_higher(),
        }
    }

    pub fn parse_spread_element(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::DotDotDotToken);
        let expression = self.parse_assignment_expression_or_higher();
        // PORT NOTE: reshaped for borrowck
        let node = self.factory.new_spread_element(expression);
        self.finish_node(node, pos)
    }

    pub fn parse_object_literal_element(&mut self) -> ast::Node {
        let pos = self.node_pos();
        if self.parse_optional(ast::Kind::DotDotDotToken) {
            let expression = self.parse_assignment_expression_or_higher();
            // PORT NOTE: reshaped for borrowck
            let result = self.factory.new_spread_assignment(expression);
            let result = self.finish_node(result, pos);
            return result;
        }
        let modifiers = self.parse_modifiers_ex(true, false, false);
        if self.parse_contextual_modifier(ast::Kind::GetKeyword) {
            return self.parse_accessor_declaration(
                pos,
                modifiers,
                ast::Kind::GetAccessor,
                crate::ParseFlags::NONE,
            );
        }
        if self.parse_contextual_modifier(ast::Kind::SetKeyword) {
            return self.parse_accessor_declaration(
                pos,
                modifiers,
                ast::Kind::SetAccessor,
                crate::ParseFlags::NONE,
            );
        }
        let asterisk_token = self.parse_optional_token(ast::Kind::AsteriskToken);
        let token_is_identifier = self.is_identifier();
        let name = self.parse_property_name();
        let mut postfix_token = self.parse_optional_token(ast::Kind::QuestionToken);
        if postfix_token.is_none() {
            postfix_token = self.parse_optional_token(ast::Kind::ExclamationToken);
        }
        let mut node = if asterisk_token.is_some()
            || self.token == ast::Kind::OpenParenToken
            || self.token == ast::Kind::LessThanToken
        {
            return self.parse_method_declaration(
                pos,
                modifiers,
                asterisk_token,
                name,
                postfix_token,
                None,
            );
        } else if token_is_identifier && self.token != ast::Kind::ColonToken {
            let equals_token = self.parse_optional_token(ast::Kind::EqualsToken);
            let initializer = if equals_token.is_some() {
                Some(do_in_context(
                    self,
                    ast::NodeFlags::DISALLOW_IN_CONTEXT,
                    false,
                    Parser::parse_assignment_expression_or_higher,
                ))
            } else {
                None
            };
            self.factory.new_shorthand_property_assignment(
                modifiers,
                name,
                postfix_token,
                None,
                equals_token,
                initializer,
            )
        } else {
            self.parse_expected(ast::Kind::ColonToken);
            let initializer = do_in_context(
                self,
                ast::NodeFlags::DISALLOW_IN_CONTEXT,
                false,
                Parser::parse_assignment_expression_or_higher,
            );
            self.factory
                .new_property_assignment(modifiers, name, postfix_token, None, initializer)
        };
        node = self.finish_node(node, pos);
        node
    }

    pub fn new_parser_node_list(
        &mut self,
        loc: core::TextRange,
        nodes: Vec<ast::Node>,
    ) -> ast::NodeList {
        self.factory.new_node_list(loc, loc, nodes)
    }

    pub fn new_parser_node_list_with_trailing_comma(
        &mut self,
        loc: core::TextRange,
        nodes: Vec<ast::Node>,
        has_trailing_comma: bool,
    ) -> ast::NodeList {
        self.factory
            .new_node_list_with_trailing_comma(loc, loc, nodes, has_trailing_comma)
    }

    pub fn new_parser_modifier_list(
        &mut self,
        loc: core::TextRange,
        nodes: Vec<ast::Node>,
    ) -> ast::ModifierList {
        let modifier_flags = ast::modifiers_to_flags(self.factory.store(), &nodes);
        self.factory
            .new_modifier_list(loc, loc, nodes, modifier_flags)
    }

    pub fn finish_node(&mut self, node: ast::Node, pos: i32) -> ast::Node {
        self.finish_node_with_end(node, pos, self.node_pos())
    }

    fn reference_directives_from_pragmas(
        &mut self,
        pragmas: &[ast::Pragma],
    ) -> (
        Vec<ast::FileReference>,
        Vec<ast::FileReference>,
        Vec<ast::FileReference>,
        Option<ast::CheckJsDirective>,
    ) {
        let mut referenced_files = Vec::new();
        let mut type_reference_directives = Vec::new();
        let mut lib_reference_directives = Vec::new();
        let mut check_js_directive = None;
        for pragma in pragmas {
            match pragma.name.as_str() {
                "reference" => {
                    let types = pragma.args.get("types");
                    let lib = pragma.args.get("lib");
                    let path = pragma.args.get("path");
                    let resolution_mode = pragma.args.get("resolution-mode");
                    let preserve = pragma
                        .args
                        .get("preserve")
                        .is_some_and(|arg| arg.value == "true");
                    let no_default_lib = pragma
                        .args
                        .get("no-default-lib")
                        .is_some_and(|arg| arg.value == "true");
                    if no_default_lib {
                        continue;
                    } else if let Some(types) = types {
                        let parsed = resolution_mode
                            .map(|mode| {
                                self.parse_resolution_mode(
                                    &mode.value,
                                    mode.text_range.pos(),
                                    mode.text_range.end(),
                                )
                            })
                            .unwrap_or_default();
                        type_reference_directives.push(ast::FileReference {
                            text_range: types.text_range,
                            file_name: types.value.clone(),
                            resolution_mode: parsed,
                            preserve,
                        });
                    } else if let Some(lib) = lib {
                        lib_reference_directives.push(ast::FileReference {
                            text_range: lib.text_range,
                            file_name: lib.value.clone(),
                            resolution_mode: core::ResolutionMode::default(),
                            preserve,
                        });
                    } else if let Some(path) = path {
                        referenced_files.push(ast::FileReference {
                            text_range: path.text_range,
                            file_name: path.value.clone(),
                            resolution_mode: core::ResolutionMode::default(),
                            preserve,
                        });
                    } else {
                        self.parse_error_at_range(
                            pragma.comment_range.text_range,
                            &diagnostics::INVALID_REFERENCE_DIRECTIVE_SYNTAX,
                            Vec::new(),
                        );
                    }
                }
                "ts-check" | "ts-nocheck" => {
                    if check_js_directive
                        .as_ref()
                        .is_none_or(|check: &ast::CheckJsDirective| {
                            pragma.comment_range.text_range.pos() > check.range.text_range.pos()
                        })
                    {
                        check_js_directive = Some(ast::CheckJsDirective {
                            enabled: pragma.name == "ts-check",
                            range: pragma.comment_range.clone(),
                        });
                    }
                }
                "jsx" | "jsxfrag" | "jsximportsource" | "jsxruntime" => {}
                _ => panic!("Unhandled pragma kind: {}", pragma.name),
            }
        }
        (
            referenced_files,
            type_reference_directives,
            lib_reference_directives,
            check_js_directive,
        )
    }

    pub fn look_ahead<T>(&mut self, callback: impl FnOnce(&mut Parser) -> T) -> T {
        let state = self.mark();
        let result = callback(self);
        self.rewind(state);
        result
    }

    pub fn has_preceding_line_break(&self) -> bool {
        self.scanner
            .as_ref()
            .expect("scanner")
            .has_preceding_line_break()
    }

    pub fn parse_toplevel_statement(&mut self, i: i32) -> ast::Node {
        self.statement_has_await_identifier = false;
        let statement = self.parse_statement();
        if self.statement_has_await_identifier
            && !self
                .factory
                .store()
                .flags(statement)
                .contains(ast::NodeFlags::AWAIT_CONTEXT)
        {
            if self.possible_await_spans.is_empty()
                || *self.possible_await_spans.last().unwrap() != i
            {
                self.possible_await_spans.push(i);
                self.possible_await_spans.push(i + 1);
            } else {
                let last = self.possible_await_spans.len() - 1;
                self.possible_await_spans[last] = i + 1;
            }
        }
        statement
    }

    pub fn reparse_top_level_await(&mut self, source_file: ast::Node) -> ast::Node {
        if self.possible_await_spans.len() % 2 == 1 {
            panic!("possibleAwaitSpans malformed: odd number of indices, not paired into spans.");
        }
        let (source_file_parse_options, source_file_end_of_file_token) = {
            let source_file_data = self.factory.store().as_source_file(source_file);
            (
                source_file_data.parse_options(),
                source_file_data.end_of_file_token(),
            )
        };
        let source_file_statement_list = self
            .factory
            .store()
            .parser_access()
            .source_file_statement_list(source_file);
        let source_file_statements = source_file_statement_list.iter().collect::<Vec<_>>();
        let source_file_statement_loc = source_file_statement_list.loc();
        let mut statements: Vec<ast::Node> = Vec::new();
        let saved_parse_diagnostics = self.diagnostics.clone();
        self.diagnostics = Vec::new();

        let mut after_await_statement = 0usize;
        let mut i = 0usize;
        while i < self.possible_await_spans.len() {
            let next_await_statement = self.possible_await_spans[i] as usize;
            let prev_statement = source_file_statements[after_await_statement];
            let next_statement = source_file_statements[next_await_statement];
            statements.extend_from_slice(
                &source_file_statements[after_await_statement..next_await_statement],
            );

            let diagnostic_start = saved_parse_diagnostics.iter().position(|diagnostic| {
                diagnostic.pos() >= self.factory.store().loc(prev_statement).pos()
            });
            if let Some(diagnostic_start) = diagnostic_start {
                let diagnostic_end =
                    saved_parse_diagnostics[..diagnostic_start]
                        .iter()
                        .position(|diagnostic| {
                            diagnostic.pos() >= self.factory.store().loc(next_statement).pos()
                        });
                if let Some(diagnostic_end) = diagnostic_end {
                    self.diagnostics.extend_from_slice(
                        &saved_parse_diagnostics
                            [diagnostic_start..diagnostic_start + diagnostic_end],
                    );
                } else {
                    self.diagnostics
                        .extend_from_slice(&saved_parse_diagnostics[diagnostic_start..]);
                }
            }

            let mut state = self.mark();
            self.context_flags |= ast::NodeFlags::AWAIT_CONTEXT;
            self.scanner
                .as_mut()
                .expect("scanner")
                .reset_pos(self.factory.store().loc(next_statement).pos());
            self.next_token();

            after_await_statement = self.possible_await_spans[i + 1] as usize;
            while self.token != ast::Kind::EndOfFile {
                let start_pos = self.scanner.as_ref().expect("scanner").token_full_start();
                let statement = self.parse_statement();
                statements.push(statement.clone());
                if start_pos == self.scanner.as_ref().expect("scanner").token_full_start() {
                    self.next_token();
                }
                if after_await_statement < source_file_statements.len() {
                    let non_await_statement = source_file_statements[after_await_statement];
                    let statement_end = self.factory.store().loc(statement).end();
                    let non_await_statement_pos =
                        self.factory.store().loc(non_await_statement).pos();
                    if statement_end == non_await_statement_pos {
                        break;
                    }
                    if statement_end > non_await_statement_pos {
                        i += 2;
                        if i < self.possible_await_spans.len() {
                            after_await_statement = self.possible_await_spans[i + 1] as usize;
                        } else {
                            after_await_statement = source_file_statements.len();
                        }
                    }
                }
            }

            state.diagnostics_len = self.diagnostics.len();
            self.rewind(state);
            i += 2;
        }

        if after_await_statement < source_file_statements.len() {
            let prev_statement = source_file_statements[after_await_statement];
            statements.extend_from_slice(&source_file_statements[after_await_statement..]);
            if let Some(diagnostic_start) = saved_parse_diagnostics.iter().position(|diagnostic| {
                diagnostic.pos() >= self.factory.store().loc(prev_statement).pos()
            }) {
                self.diagnostics
                    .extend_from_slice(&saved_parse_diagnostics[diagnostic_start..]);
            }
        }

        // PORT NOTE: reshaped for borrowck
        let statement_list =
            self.new_parser_node_list(source_file_statement_loc, statements.clone());
        let result = self.factory.new_source_file(
            source_file_parse_options,
            Arc::clone(&self.source_text),
            statement_list,
            source_file_end_of_file_token,
        );
        for statement in statements {
            self.factory.link_parsed_parent(statement, Some(result));
        }
        result
    }

    pub fn parse_list_index(
        &mut self,
        kind: ParsingContext,
        mut parse_element: impl FnMut(&mut Parser, i32) -> ast::Node,
    ) -> Vec<ast::Node> {
        let save_parsing_contexts = self.parsing_contexts;
        self.parsing_contexts |= 1 << (kind as i32);
        let mut outer_reparse_list = std::mem::take(&mut self.reparse_list);
        let mut list = Vec::with_capacity(16);
        for _ in 0.. {
            if self.is_list_terminator(kind) {
                break;
            }
            if self.is_list_element(kind, false) {
                let elt = parse_element(self, list.len() as i32);
                if !self.reparse_list.is_empty() {
                    for e in self.reparse_list.drain(..) {
                        if (ast::is_js_type_alias_declaration(self.factory.store(), e)
                            || ast::is_js_import_declaration(self.factory.store(), e))
                            && kind != ParsingContext::PCSourceElements
                            && kind != ParsingContext::PCBlockStatements
                        {
                            outer_reparse_list.push(e);
                        } else {
                            list.push(e);
                        }
                    }
                }
                list.push(elt);
                continue;
            }
            if self.abort_parsing_list_or_move_to_next_token(kind) {
                break;
            }
        }
        self.reparse_list = outer_reparse_list;
        self.parsing_contexts = save_parsing_contexts;
        list
    }

    pub fn parse_delimited_list(
        &mut self,
        kind: ParsingContext,
        parse_element: fn(&mut Parser) -> ast::Node,
    ) -> ast::NodeList {
        self.parse_delimited_list_opt(kind, |parser| Some(parse_element(parser)))
            .expect("non-optional parse element")
    }

    pub fn parse_delimited_list_opt(
        &mut self,
        kind: ParsingContext,
        mut parse_element: impl FnMut(&mut Parser) -> Option<ast::Node>,
    ) -> Option<ast::NodeList> {
        let pos = self.node_pos();
        let save_parsing_contexts = self.parsing_contexts;
        self.parsing_contexts |= 1 << (kind as i32);
        let mut list = Vec::with_capacity(16);
        let mut comma_start = -1;
        loop {
            if self.is_list_element(kind, false) {
                let start_pos = self.node_pos();
                let Some(element) = parse_element(self) else {
                    self.parsing_contexts = save_parsing_contexts;
                    return None;
                };
                list.push(element);
                comma_start = self.scanner.as_ref().expect("scanner").token_start();
                if self.parse_optional(ast::Kind::CommaToken) {
                    // No need to check for a zero length node since we know we parsed a comma
                    continue;
                }
                comma_start = -1;
                if self.is_list_terminator(kind) {
                    break;
                }
                // We didn't get a comma, and the list wasn't terminated, explicitly parse
                // out a comma so we give a good error message.
                if self.token != ast::Kind::CommaToken && kind == ParsingContext::PCEnumMembers {
                    self.parse_error_at_current_token(
                        &diagnostics::AN_ENUM_MEMBER_NAME_MUST_BE_FOLLOWED_BY_A_OR,
                        Vec::new(),
                    );
                } else {
                    self.parse_expected(ast::Kind::CommaToken);
                }
                // If the token was a semicolon, and the caller allows that, then skip it and
                // continue.  This ensures we get back on track and don't result in tons of
                // parse errors.  For example, this can happen when people do things like use
                // a semicolon to delimit object literal members.   Note: we'll have already
                // reported an error when we called parseExpected above.
                if (kind == ParsingContext::PCObjectLiteralMembers
                    || kind == ParsingContext::PCImportAttributes)
                    && self.token == ast::Kind::SemicolonToken
                    && !self.has_preceding_line_break()
                {
                    comma_start = self.scanner.as_ref().expect("scanner").token_start();
                    self.next_token();
                }
                if start_pos == self.node_pos() {
                    // What we're parsing isn't actually remotely recognizable as a element and we've consumed no tokens whatsoever
                    // Consume a token to advance the parser in some way and avoid an infinite loop
                    // This can happen when we're speculatively parsing parenthesized expressions which we think may be arrow functions,
                    // or when a modifier keyword which is disallowed as a parameter name (ie, `static` in strict mode) is supplied
                    self.next_token();
                }
                continue;
            }
            if self.is_list_terminator(kind) {
                break;
            }
            if self.abort_parsing_list_or_move_to_next_token(kind) {
                break;
            }
        }
        self.parsing_contexts = save_parsing_contexts;
        let end = self.node_pos();
        let has_trailing_comma = comma_start >= 0
            || list
                .last()
                .is_some_and(|last| self.factory.store().loc(*last).end() < end);
        Some(self.new_parser_node_list_with_trailing_comma(
            core::TextRange::new(pos, end),
            list,
            has_trailing_comma,
        ))
    }

    pub fn parse_list(
        &mut self,
        kind: ParsingContext,
        mut parse_element: impl FnMut(&mut Parser) -> ast::Node,
    ) -> ast::NodeList {
        let pos = self.node_pos();
        let nodes = self.parse_list_index(kind, |p, _| parse_element(p));
        self.new_parser_node_list(core::TextRange::new(pos, self.node_pos()), nodes)
    }

    pub fn parse_bracketed_list(
        &mut self,
        kind: ParsingContext,
        parse_element: fn(&mut Parser) -> ast::Node,
        opening: ast::Kind,
        closing: ast::Kind,
    ) -> ast::NodeList {
        if self.parse_expected(opening) {
            let result = self.parse_delimited_list(kind, parse_element);
            self.parse_expected(closing);
            return result;
        }
        self.create_missing_list()
    }

    pub fn parse_empty_node_list(&mut self) -> ast::NodeList {
        self.new_parser_node_list(
            core::TextRange::new(self.node_pos(), self.node_pos()),
            Vec::new(),
        )
    }

    pub fn create_missing_list(&mut self) -> ast::NodeList {
        let loc = core::TextRange::new(self.node_pos(), self.node_pos());
        self.factory.new_missing_node_list(loc, loc)
    }

    pub fn abort_parsing_list_or_move_to_next_token(&mut self, kind: ParsingContext) -> bool {
        self.parsing_context_errors(kind);
        if self.is_in_some_parsing_context() {
            return true;
        }
        self.next_token();
        false
    }

    pub fn parsing_context_errors(&mut self, context: ParsingContext) {
        match context {
            ParsingContext::PCSourceElements => {
                if self.token == ast::Kind::DefaultKeyword {
                    self.parse_error_at_current_token(
                        &diagnostics::X_0_EXPECTED,
                        vec![diagnostics::Any::from("export".to_string())],
                    );
                } else {
                    self.parse_error_at_current_token(
                        &diagnostics::DECLARATION_OR_STATEMENT_EXPECTED,
                        Vec::new(),
                    );
                }
            }
            ParsingContext::PCBlockStatements => {
                self.parse_error_at_current_token(
                    &diagnostics::DECLARATION_OR_STATEMENT_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCSwitchClauses => {
                self.parse_error_at_current_token(
                    &diagnostics::X_CASE_OR_DEFAULT_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCSwitchClauseStatements => {
                self.parse_error_at_current_token(&diagnostics::STATEMENT_EXPECTED, Vec::new());
            }
            ParsingContext::PCRestProperties | ParsingContext::PCTypeMembers => {
                self.parse_error_at_current_token(
                    &diagnostics::PROPERTY_OR_SIGNATURE_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCClassMembers => {
                self.parse_error_at_current_token(
                    &diagnostics::UNEXPECTED_TOKEN_A_CONSTRUCTOR_METHOD_ACCESSOR_OR_PROPERTY_WAS_EXPECTED,
                    Vec::new());
            }
            ParsingContext::PCEnumMembers => {
                self.parse_error_at_current_token(&diagnostics::ENUM_MEMBER_EXPECTED, Vec::new());
            }
            ParsingContext::PCHeritageClauseElement => {
                self.parse_error_at_current_token(&diagnostics::EXPRESSION_EXPECTED, Vec::new());
            }
            ParsingContext::PCVariableDeclarations => {
                if ast::is_keyword(self.token) {
                    self.parse_error_at_current_token(
                        &diagnostics::X_0_IS_NOT_ALLOWED_AS_A_VARIABLE_DECLARATION_NAME,
                        vec![diagnostics::Any::from(scanner::token_to_string(self.token))],
                    );
                } else {
                    self.parse_error_at_current_token(
                        &diagnostics::VARIABLE_DECLARATION_EXPECTED,
                        Vec::new(),
                    );
                }
            }
            ParsingContext::PCObjectBindingElements => {
                self.parse_error_at_current_token(
                    &diagnostics::PROPERTY_DESTRUCTURING_PATTERN_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCArrayBindingElements => {
                self.parse_error_at_current_token(
                    &diagnostics::ARRAY_ELEMENT_DESTRUCTURING_PATTERN_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCArgumentExpressions => {
                self.parse_error_at_current_token(
                    &diagnostics::ARGUMENT_EXPRESSION_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCObjectLiteralMembers => {
                self.parse_error_at_current_token(
                    &diagnostics::PROPERTY_ASSIGNMENT_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCArrayLiteralMembers => {
                self.parse_error_at_current_token(
                    &diagnostics::EXPRESSION_OR_COMMA_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCParameters => {
                if ast::is_keyword(self.token) {
                    self.parse_error_at_current_token(
                        &diagnostics::X_0_IS_NOT_ALLOWED_AS_A_PARAMETER_NAME,
                        vec![diagnostics::Any::from(scanner::token_to_string(self.token))],
                    );
                } else {
                    self.parse_error_at_current_token(
                        &diagnostics::PARAMETER_DECLARATION_EXPECTED,
                        Vec::new(),
                    );
                }
            }
            ParsingContext::PCTypeParameters => {
                self.parse_error_at_current_token(
                    &diagnostics::TYPE_PARAMETER_DECLARATION_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCTypeArguments => {
                self.parse_error_at_current_token(&diagnostics::TYPE_ARGUMENT_EXPECTED, Vec::new());
            }
            ParsingContext::PCTupleElementTypes => {
                self.parse_error_at_current_token(&diagnostics::TYPE_EXPECTED, Vec::new());
            }
            ParsingContext::PCHeritageClauses => {
                self.parse_error_at_current_token(
                    &diagnostics::UNEXPECTED_TOKEN_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCImportOrExportSpecifiers => {
                if self.token == ast::Kind::FromKeyword {
                    self.parse_error_at_current_token(
                        &diagnostics::X_0_EXPECTED,
                        vec![diagnostics::Any::from("}".to_string())],
                    );
                } else {
                    self.parse_error_at_current_token(
                        &diagnostics::IDENTIFIER_EXPECTED,
                        Vec::new(),
                    );
                }
            }
            ParsingContext::PCJsxAttributes | ParsingContext::PCJsxChildren => {
                self.parse_error_at_current_token(&diagnostics::IDENTIFIER_EXPECTED, Vec::new());
            }
            ParsingContext::PCImportAttributes => {
                self.parse_error_at_current_token(
                    &diagnostics::IDENTIFIER_OR_STRING_LITERAL_EXPECTED,
                    Vec::new(),
                );
            }
            ParsingContext::PCCount => panic!("Unhandled case in parsingContextErrors"),
        };
    }

    pub fn is_in_some_parsing_context(&mut self) -> bool {
        debug::assert(
            self.parsing_contexts != 0,
            Some("Missing parsing context".to_string()),
        );
        for kind in 0..(ParsingContext::PCCount as i32) {
            let context = parsing_context_from_i32(kind);
            if self.parsing_contexts & (1 << kind) != 0
                && (self.is_list_element(context, true) || self.is_list_terminator(context))
            {
                return true;
            }
        }
        false
    }

    pub fn is_list_element(
        &mut self,
        parsing_context: ParsingContext,
        in_error_recovery: bool,
    ) -> bool {
        match parsing_context {
            ParsingContext::PCSourceElements
            | ParsingContext::PCBlockStatements
            | ParsingContext::PCSwitchClauseStatements => {
                !(self.token == ast::Kind::SemicolonToken && in_error_recovery)
                    && self.is_start_of_statement()
            }
            ParsingContext::PCSwitchClauses => {
                self.token == ast::Kind::CaseKeyword || self.token == ast::Kind::DefaultKeyword
            }
            ParsingContext::PCTypeMembers => self.look_ahead(Parser::scan_type_member_start),
            ParsingContext::PCClassMembers => {
                self.look_ahead(Parser::scan_class_member_start)
                    || self.token == ast::Kind::SemicolonToken && !in_error_recovery
            }
            ParsingContext::PCEnumMembers => {
                self.token == ast::Kind::OpenBracketToken || self.is_literal_property_name()
            }
            ParsingContext::PCObjectLiteralMembers => match self.token {
                ast::Kind::OpenBracketToken
                | ast::Kind::AsteriskToken
                | ast::Kind::DotDotDotToken
                | ast::Kind::DotToken => true,
                _ => self.is_literal_property_name(),
            },
            ParsingContext::PCRestProperties => self.is_literal_property_name(),
            ParsingContext::PCObjectBindingElements => {
                self.token == ast::Kind::OpenBracketToken
                    || self.token == ast::Kind::DotDotDotToken
                    || self.is_literal_property_name()
            }
            ParsingContext::PCImportAttributes => self.is_import_attribute_name(),
            ParsingContext::PCHeritageClauseElement => {
                if self.token == ast::Kind::OpenBraceToken {
                    return self.is_valid_heritage_clause_object_literal();
                }
                if !in_error_recovery {
                    return self.is_start_of_left_hand_side_expression()
                        && !self.is_heritage_clause_extends_or_implements_keyword();
                }
                self.is_identifier() && !self.is_heritage_clause_extends_or_implements_keyword()
            }
            ParsingContext::PCVariableDeclarations => {
                self.is_binding_identifier_or_private_identifier_or_pattern()
            }
            ParsingContext::PCArrayBindingElements => {
                self.token == ast::Kind::CommaToken
                    || self.token == ast::Kind::DotDotDotToken
                    || self.is_binding_identifier_or_private_identifier_or_pattern()
            }
            ParsingContext::PCTypeParameters => {
                self.token == ast::Kind::InKeyword
                    || self.token == ast::Kind::ConstKeyword
                    || self.is_identifier()
            }
            ParsingContext::PCArrayLiteralMembers => {
                if self.token == ast::Kind::CommaToken || self.token == ast::Kind::DotToken {
                    return true;
                }
                self.token == ast::Kind::DotDotDotToken || self.is_start_of_expression()
            }
            ParsingContext::PCArgumentExpressions => {
                self.token == ast::Kind::DotDotDotToken || self.is_start_of_expression()
            }
            ParsingContext::PCParameters => self.is_start_of_parameter(),
            ParsingContext::PCTypeArguments | ParsingContext::PCTupleElementTypes => {
                self.token == ast::Kind::CommaToken || self.is_start_of_type(false)
            }
            ParsingContext::PCHeritageClauses => self.is_heritage_clause(),
            ParsingContext::PCImportOrExportSpecifiers => {
                if self.token == ast::Kind::FromKeyword
                    && self.look_ahead(Parser::next_token_is_token_string_literal)
                {
                    return false;
                }
                if self.token == ast::Kind::StringLiteral {
                    return true;
                }
                token_is_identifier_or_keyword(self.token)
            }
            ParsingContext::PCJsxAttributes => {
                token_is_identifier_or_keyword(self.token)
                    || self.token == ast::Kind::OpenBraceToken
            }
            ParsingContext::PCJsxChildren => true,
            ParsingContext::PCCount => panic!("Unhandled case in isListElement"),
        }
    }

    pub fn is_list_terminator(&mut self, kind: ParsingContext) -> bool {
        if self.token == ast::Kind::EndOfFile {
            return true;
        }
        match kind {
            ParsingContext::PCBlockStatements
            | ParsingContext::PCSwitchClauses
            | ParsingContext::PCTypeMembers
            | ParsingContext::PCClassMembers
            | ParsingContext::PCEnumMembers
            | ParsingContext::PCObjectLiteralMembers
            | ParsingContext::PCObjectBindingElements
            | ParsingContext::PCImportOrExportSpecifiers
            | ParsingContext::PCImportAttributes => self.token == ast::Kind::CloseBraceToken,
            ParsingContext::PCSwitchClauseStatements => {
                self.token == ast::Kind::CloseBraceToken
                    || self.token == ast::Kind::CaseKeyword
                    || self.token == ast::Kind::DefaultKeyword
            }
            ParsingContext::PCHeritageClauseElement => {
                self.token == ast::Kind::OpenBraceToken
                    || self.token == ast::Kind::ExtendsKeyword
                    || self.token == ast::Kind::ImplementsKeyword
            }
            ParsingContext::PCVariableDeclarations => {
                self.can_parse_semicolon()
                    || self.token == ast::Kind::InKeyword
                    || self.token == ast::Kind::OfKeyword
                    || self.token == ast::Kind::EqualsGreaterThanToken
            }
            ParsingContext::PCTypeParameters => {
                self.token == ast::Kind::GreaterThanToken
                    || self.token == ast::Kind::OpenParenToken
                    || self.token == ast::Kind::OpenBraceToken
                    || self.token == ast::Kind::ExtendsKeyword
                    || self.token == ast::Kind::ImplementsKeyword
            }
            ParsingContext::PCArgumentExpressions => {
                self.token == ast::Kind::CloseParenToken || self.token == ast::Kind::SemicolonToken
            }
            ParsingContext::PCArrayLiteralMembers
            | ParsingContext::PCTupleElementTypes
            | ParsingContext::PCArrayBindingElements => self.token == ast::Kind::CloseBracketToken,
            ParsingContext::PCParameters | ParsingContext::PCRestProperties => {
                self.token == ast::Kind::CloseParenToken
                    || self.token == ast::Kind::CloseBracketToken
            }
            ParsingContext::PCTypeArguments => self.token != ast::Kind::CommaToken,
            ParsingContext::PCHeritageClauses => {
                self.token == ast::Kind::OpenBraceToken || self.token == ast::Kind::CloseBraceToken
            }
            ParsingContext::PCJsxAttributes => {
                self.token == ast::Kind::GreaterThanToken || self.token == ast::Kind::SlashToken
            }
            ParsingContext::PCJsxChildren => {
                self.token == ast::Kind::LessThanToken
                    && self.look_ahead(Parser::next_token_is_slash)
            }
            _ => false,
        }
    }

    pub fn parse_optional(&mut self, token: ast::Kind) -> bool {
        if self.token == token {
            self.next_token();
            return true;
        }
        false
    }

    pub fn parse_expected(&mut self, kind: ast::Kind) -> bool {
        self.parse_expected_with_diagnostic(kind, None, true)
    }

    pub fn parse_expected_without_advancing(&mut self, kind: ast::Kind) -> bool {
        self.parse_expected_with_diagnostic(kind, None, false)
    }

    pub fn parse_expected_with_diagnostic(
        &mut self,
        kind: ast::Kind,
        message: Option<&diagnostics::Message>,
        should_advance: bool,
    ) -> bool {
        if self.token == kind {
            if should_advance {
                self.next_token();
            }
            return true;
        }
        if let Some(message) = message {
            self.parse_error_at_current_token(message, Vec::new());
        } else {
            self.parse_error_at_current_token(
                &diagnostics::X_0_EXPECTED,
                vec![diagnostics::Any::from(scanner::token_to_string(kind))],
            );
        }
        false
    }

    pub fn parse_optional_token(&mut self, kind: ast::Kind) -> Option<ast::Node> {
        if self.token == kind {
            Some(self.parse_token_node())
        } else {
            None
        }
    }

    pub fn parse_expected_matching_brackets(
        &mut self,
        open_kind: ast::Kind,
        close_kind: ast::Kind,
        open_parsed: bool,
        open_position: i32,
    ) {
        if self.token == close_kind {
            self.next_token();
            return;
        }
        let diagnostic_count = self.diagnostics.len();
        self.parse_error_at_current_token(
            &diagnostics::X_0_EXPECTED,
            vec![diagnostics::Any::from(scanner::token_to_string(close_kind))],
        );
        if !open_parsed {
            return;
        }
        if self.diagnostics.len() != diagnostic_count {
            self.add_bracket_related_info(open_position, open_kind, close_kind);
        }
    }

    pub fn parse_statement(&mut self) -> ast::Node {
        match self.token {
            ast::Kind::SemicolonToken => return self.parse_empty_statement(),
            ast::Kind::OpenBraceToken => return self.parse_block(false, None),
            ast::Kind::VarKeyword => {
                return self.parse_variable_statement(self.node_pos(), None);
            }
            ast::Kind::LetKeyword => {
                if self.is_let_declaration() {
                    return self.parse_variable_statement(self.node_pos(), None);
                }
            }
            ast::Kind::AwaitKeyword => {
                if self.is_await_using_declaration() {
                    return self.parse_variable_statement(self.node_pos(), None);
                }
            }
            ast::Kind::UsingKeyword => {
                if self.is_using_declaration() {
                    return self.parse_variable_statement(self.node_pos(), None);
                }
            }
            ast::Kind::FunctionKeyword => {
                return self.parse_function_declaration(self.node_pos(), None);
            }
            ast::Kind::ClassKeyword => {
                return self.parse_class_declaration(self.node_pos(), None);
            }
            ast::Kind::IfKeyword => return self.parse_if_statement(),
            ast::Kind::DoKeyword => return self.parse_do_statement(),
            ast::Kind::WhileKeyword => return self.parse_while_statement(),
            ast::Kind::ForKeyword => return self.parse_for_or_for_in_or_for_of_statement(),
            ast::Kind::ContinueKeyword => return self.parse_continue_statement(),
            ast::Kind::BreakKeyword => return self.parse_break_statement(),
            ast::Kind::ReturnKeyword => return self.parse_return_statement(),
            ast::Kind::WithKeyword => return self.parse_with_statement(),
            ast::Kind::SwitchKeyword => return self.parse_switch_statement(),
            ast::Kind::ThrowKeyword => return self.parse_throw_statement(),
            ast::Kind::TryKeyword | ast::Kind::CatchKeyword | ast::Kind::FinallyKeyword => {
                return self.parse_try_statement();
            }
            ast::Kind::DebuggerKeyword => return self.parse_debugger_statement(),
            ast::Kind::AtToken => return self.parse_declaration(),
            ast::Kind::AsyncKeyword
            | ast::Kind::InterfaceKeyword
            | ast::Kind::TypeKeyword
            | ast::Kind::ModuleKeyword
            | ast::Kind::NamespaceKeyword
            | ast::Kind::DeclareKeyword
            | ast::Kind::ConstKeyword
            | ast::Kind::EnumKeyword
            | ast::Kind::ExportKeyword
            | ast::Kind::ImportKeyword
            | ast::Kind::PrivateKeyword
            | ast::Kind::ProtectedKeyword
            | ast::Kind::PublicKeyword
            | ast::Kind::AbstractKeyword
            | ast::Kind::AccessorKeyword
            | ast::Kind::StaticKeyword
            | ast::Kind::ReadonlyKeyword
            | ast::Kind::GlobalKeyword => {
                if self.is_start_of_declaration() {
                    return self.parse_declaration();
                }
            }
            _ => {}
        }
        self.parse_expression_or_labeled_statement()
    }

    pub fn parse_declaration(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers_ex(true, false, false);
        let is_ambient = modifiers.as_ref().is_some_and(|modifiers| {
            self.factory
                .store()
                .parser_access()
                .modifier_list_nodes(*modifiers)
                .into_iter()
                .any(|modifier| is_declare_modifier(self.factory.store(), &modifier))
        });
        if is_ambient {
            if let Some(modifiers) = modifiers {
                let modifiers = self.factory.parsed_modifier_list_nodes(modifiers);
                for modifier in modifiers {
                    self.factory.mark_parsed_modifier_ambient(modifier);
                }
            }
            let save_context_flags = self.context_flags;
            self.set_context_flags(ast::NodeFlags::AMBIENT, true);
            let result = self.parse_declaration_worker(pos, modifiers);
            self.context_flags = save_context_flags;
            result
        } else {
            self.parse_declaration_worker(pos, modifiers)
        }
    }

    pub fn parse_declaration_worker(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        match self.token {
            ast::Kind::VarKeyword
            | ast::Kind::LetKeyword
            | ast::Kind::ConstKeyword
            | ast::Kind::UsingKeyword => {
                return self.parse_variable_statement(pos, modifiers);
            }
            ast::Kind::AwaitKeyword => {
                if self.is_await_using_declaration() {
                    return self.parse_variable_statement(pos, modifiers);
                }
            }
            ast::Kind::FunctionKeyword => {
                return self.parse_function_declaration(pos, modifiers);
            }
            ast::Kind::ClassKeyword => return self.parse_class_declaration(pos, modifiers),
            ast::Kind::InterfaceKeyword => {
                return self.parse_interface_declaration(pos, modifiers);
            }
            ast::Kind::TypeKeyword => {
                return self.parse_type_alias_declaration(pos, modifiers);
            }
            ast::Kind::EnumKeyword => return self.parse_enum_declaration(pos, modifiers),
            ast::Kind::GlobalKeyword | ast::Kind::ModuleKeyword | ast::Kind::NamespaceKeyword => {
                return self.parse_module_declaration(pos, modifiers);
            }
            ast::Kind::ImportKeyword => {
                return self.parse_import_declaration_or_import_equals_declaration(pos, modifiers);
            }
            ast::Kind::ExportKeyword => {
                self.next_token();
                return match self.token {
                    ast::Kind::DefaultKeyword | ast::Kind::EqualsToken => {
                        self.parse_export_assignment(pos, modifiers)
                    }
                    ast::Kind::AsKeyword => self.parse_namespace_export_declaration(pos, modifiers),
                    _ => self.parse_export_declaration(pos, modifiers),
                };
            }
            _ => {}
        }
        if modifiers.is_some() {
            self.parse_error_at(
                self.node_pos(),
                self.node_pos(),
                &diagnostics::DECLARATION_EXPECTED,
                Vec::new(),
            );
            return finish_node!(self, self.factory.new_missing_declaration(modifiers), pos);
        }
        panic!("Unhandled case in parseDeclarationWorker")
    }

    pub fn is_let_declaration(&mut self) -> bool {
        self.look_ahead(Parser::next_token_is_binding_identifier_or_start_of_destructuring)
    }

    pub fn next_token_is_binding_identifier_or_start_of_destructuring(&mut self) -> bool {
        self.next_token();
        self.is_binding_identifier()
            || self.token == ast::Kind::OpenBraceToken
            || self.token == ast::Kind::OpenBracketToken
    }

    pub fn parse_block(
        &mut self,
        ignore_missing_open_brace: bool,
        diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        let pos = self.node_pos();
        let open_brace_position = self.scanner.as_ref().expect("scanner").token_start();
        let open_brace_parsed = self.parse_expected_with_diagnostic(
            ast::Kind::OpenBraceToken,
            diagnostic_message,
            true,
        );
        let mut multiline = false;
        let result = if open_brace_parsed || ignore_missing_open_brace {
            multiline = self.has_preceding_line_break();
            let statements =
                self.parse_list(ParsingContext::PCBlockStatements, Parser::parse_statement);
            self.parse_expected_matching_brackets(
                ast::Kind::OpenBraceToken,
                ast::Kind::CloseBraceToken,
                open_brace_parsed,
                open_brace_position,
            );
            let result = finish_node!(self, self.factory.new_block(statements, multiline), pos);
            if self.token == ast::Kind::EqualsToken {
                self.parse_error_at_current_token(
                    &diagnostics::DECLARATION_OR_STATEMENT_EXPECTED_THIS_FOLLOWS_A_BLOCK_OF_STATEMENTS_SO_IF_YOU_INTENDED_TO_WRITE_A_DESTRUCTURING_ASSIGNMENT_YOU_MIGHT_NEED_TO_WRAP_THE_WHOLE_ASSIGNMENT_IN_PARENTHESES,
                    Vec::new());
                self.next_token();
            }
            result
        } else {
            // PORT NOTE: reshaped for borrowck
            let statements = self.create_missing_list();
            let result = finish_node!(self, self.factory.new_block(statements, multiline), pos);
            result
        };
        result
    }

    pub fn parse_empty_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::SemicolonToken);
        let result = finish_node!(self, self.factory.new_empty_statement(), pos);
        result
    }

    pub fn parse_if_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::IfKeyword);
        let open_paren_position = self.scanner.as_ref().expect("scanner").token_start();
        let open_paren_parsed = self.parse_expected(ast::Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            ast::Kind::OpenParenToken,
            ast::Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        let then_statement = self.parse_statement();
        let else_statement = if self.parse_optional(ast::Kind::ElseKeyword) {
            Some(self.parse_statement())
        } else {
            None
        };
        let result = finish_node!(
            self,
            self.factory
                .new_if_statement(expression, then_statement, else_statement),
            pos
        );
        result
    }

    pub fn parse_do_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::DoKeyword);
        let statement = self.parse_statement();
        self.parse_expected(ast::Kind::WhileKeyword);
        let open_paren_position = self.scanner.as_ref().expect("scanner").token_start();
        let open_paren_parsed = self.parse_expected(ast::Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            ast::Kind::OpenParenToken,
            ast::Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        self.parse_optional(ast::Kind::SemicolonToken);
        let result = finish_node!(
            self,
            self.factory.new_do_statement(statement, expression),
            pos
        );
        result
    }

    pub fn parse_while_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::WhileKeyword);
        let open_paren_position = self.scanner.as_ref().expect("scanner").token_start();
        let open_paren_parsed = self.parse_expected(ast::Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            ast::Kind::OpenParenToken,
            ast::Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        let statement = self.parse_statement();
        let result = finish_node!(
            self,
            self.factory.new_while_statement(expression, statement),
            pos
        );
        result
    }

    pub fn parse_break_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::BreakKeyword);
        let label = self.parse_identifier_unless_at_semicolon();
        self.parse_semicolon();
        let result = finish_node!(self, self.factory.new_break_statement(label), pos);
        result
    }

    pub fn parse_continue_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::ContinueKeyword);
        let label = self.parse_identifier_unless_at_semicolon();
        self.parse_semicolon();
        let result = finish_node!(self, self.factory.new_continue_statement(label), pos);
        result
    }

    pub fn parse_identifier_unless_at_semicolon(&mut self) -> Option<ast::Node> {
        if !self.can_parse_semicolon() {
            return Some(self.parse_identifier());
        }
        None
    }

    pub fn parse_return_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::ReturnKeyword);
        let expression = if !self.can_parse_semicolon() {
            Some(self.parse_expression_allow_in())
        } else {
            None
        };
        self.parse_semicolon();
        let result = finish_node!(self, self.factory.new_return_statement(expression), pos);
        result
    }

    pub fn parse_with_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::WithKeyword);
        let open_paren_position = self.scanner.as_ref().expect("scanner").token_start();
        let open_paren_parsed = self.parse_expected(ast::Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            ast::Kind::OpenParenToken,
            ast::Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        let statement = do_in_context(
            self,
            ast::NodeFlags::IN_WITH_STATEMENT,
            true,
            Parser::parse_statement,
        );
        let result = finish_node!(
            self,
            self.factory.new_with_statement(expression, statement),
            pos
        );
        result
    }

    pub fn parse_case_clause(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::CaseKeyword);
        let expression = self.parse_expression_allow_in();
        self.parse_expected(ast::Kind::ColonToken);
        let statements = self.parse_list(
            ParsingContext::PCSwitchClauseStatements,
            Parser::parse_statement,
        );
        let result = finish_node!(
            self,
            self.factory.new_case_or_default_clause(
                ast::Kind::CaseClause,
                Some(expression),
                statements
            ),
            pos
        );
        result
    }

    pub fn parse_default_clause(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::DefaultKeyword);
        self.parse_expected(ast::Kind::ColonToken);
        let statements = self.parse_list(
            ParsingContext::PCSwitchClauseStatements,
            Parser::parse_statement,
        );
        let result = finish_node!(
            self,
            self.factory
                .new_case_or_default_clause(ast::Kind::DefaultClause, None, statements),
            pos
        );
        result
    }

    pub fn parse_case_or_default_clause(&mut self) -> ast::Node {
        if self.token == ast::Kind::CaseKeyword {
            self.parse_case_clause()
        } else {
            self.parse_default_clause()
        }
    }

    pub fn parse_case_block(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenBraceToken);
        let clauses = self.parse_list(
            ParsingContext::PCSwitchClauses,
            Parser::parse_case_or_default_clause,
        );
        self.parse_expected(ast::Kind::CloseBraceToken);
        let result = finish_node!(self, self.factory.new_case_block(clauses), pos);
        result
    }

    pub fn parse_switch_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::SwitchKeyword);
        self.parse_expected(ast::Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected(ast::Kind::CloseParenToken);
        let case_block = self.parse_case_block();
        let result = finish_node!(
            self,
            self.factory.new_switch_statement(expression, case_block),
            pos
        );
        result
    }

    pub fn parse_for_or_for_in_or_for_of_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::ForKeyword);
        let await_token = self.parse_optional_token(ast::Kind::AwaitKeyword);
        self.parse_expected(ast::Kind::OpenParenToken);
        let initializer = if self.token != ast::Kind::SemicolonToken {
            if self.token == ast::Kind::VarKeyword
                || self.token == ast::Kind::LetKeyword
                || self.token == ast::Kind::ConstKeyword
                || self.token == ast::Kind::UsingKeyword
                    && self.look_ahead(Parser::next_token_is_binding_identifier_or_start_of_destructuring_on_same_line_disallow_of)
                || self.token == ast::Kind::AwaitKeyword
                    && self.look_ahead(Parser::next_is_using_keyword_then_binding_identifier_or_start_of_object_destructuring_on_same_line)
            {
                Some(self.parse_variable_declaration_list(true))
            } else {
                Some(do_in_context(
                    self,
                    ast::NodeFlags::DISALLOW_IN_CONTEXT,
                    true,
                    Parser::parse_expression))
            }
        } else {
            None
        };
        let result = if (await_token.is_some() && self.parse_expected(ast::Kind::OfKeyword))
            || (await_token.is_none() && self.parse_optional(ast::Kind::OfKeyword))
        {
            let expression = do_in_context(
                self,
                ast::NodeFlags::DISALLOW_IN_CONTEXT,
                false,
                Parser::parse_assignment_expression_or_higher,
            );
            self.parse_expected(ast::Kind::CloseParenToken);
            // PORT NOTE: reshaped for borrowck
            let statement = self.parse_statement();
            self.factory.new_for_in_or_of_statement(
                ast::Kind::ForOfStatement,
                await_token,
                initializer,
                expression,
                statement,
            )
        } else if self.parse_optional(ast::Kind::InKeyword) {
            let expression = self.parse_expression_allow_in();
            self.parse_expected(ast::Kind::CloseParenToken);
            // PORT NOTE: reshaped for borrowck
            let statement = self.parse_statement();
            self.factory.new_for_in_or_of_statement(
                ast::Kind::ForInStatement,
                None,
                initializer,
                expression,
                statement,
            )
        } else {
            self.parse_expected(ast::Kind::SemicolonToken);
            let condition = if self.token != ast::Kind::SemicolonToken
                && self.token != ast::Kind::CloseParenToken
            {
                Some(self.parse_expression_allow_in())
            } else {
                None
            };
            self.parse_expected(ast::Kind::SemicolonToken);
            let incrementor = if self.token != ast::Kind::CloseParenToken {
                Some(self.parse_expression_allow_in())
            } else {
                None
            };
            self.parse_expected(ast::Kind::CloseParenToken);
            // PORT NOTE: reshaped for borrowck
            let statement = self.parse_statement();
            self.factory
                .new_for_statement(initializer, condition, incrementor, statement)
        };
        let result = self.finish_node(result, pos);
        result
    }

    pub fn parse_throw_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::ThrowKeyword);
        let expression = if !self.has_preceding_line_break() {
            self.parse_expression_allow_in()
        } else {
            self.create_missing_identifier()
        };
        if !self.try_parse_semicolon() {
            self.parse_error_for_missing_semicolon_after(&expression);
        }
        let result = finish_node!(self, self.factory.new_throw_statement(expression), pos);
        result
    }

    pub fn parse_try_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::TryKeyword);
        let try_block = self.parse_block(false, None);
        let catch_clause = if self.token == ast::Kind::CatchKeyword {
            Some(self.parse_catch_clause())
        } else {
            None
        };
        let finally_block = if catch_clause.is_none() || self.token == ast::Kind::FinallyKeyword {
            self.parse_expected_with_diagnostic(
                ast::Kind::FinallyKeyword,
                Some(&diagnostics::X_CATCH_OR_FINALLY_EXPECTED),
                true,
            );
            Some(self.parse_block(false, None))
        } else {
            None
        };
        let result = finish_node!(
            self,
            self.factory
                .new_try_statement(try_block, catch_clause, finally_block),
            pos
        );
        result
    }

    pub fn parse_catch_clause(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::CatchKeyword);
        let variable_declaration = if self.parse_optional(ast::Kind::OpenParenToken) {
            let variable_declaration = self.parse_variable_declaration();
            self.parse_expected(ast::Kind::CloseParenToken);
            Some(variable_declaration)
        } else {
            None
        };
        let block = self.parse_block(false, None);
        finish_node!(
            self,
            self.factory.new_catch_clause(variable_declaration, block),
            pos
        )
    }

    pub fn parse_debugger_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::DebuggerKeyword);
        self.parse_semicolon();
        let result = finish_node!(self, self.factory.new_debugger_statement(), pos);
        result
    }

    pub fn parse_expression_or_labeled_statement(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let _has_paren = self.token == ast::Kind::OpenParenToken;
        let expression = self.parse_expression();
        if self.factory.store().kind(expression) == ast::Kind::Identifier
            && self.parse_optional(ast::Kind::ColonToken)
        {
            // PORT NOTE: reshaped for borrowck
            let statement = self.parse_statement();
            let result = finish_node!(
                self,
                self.factory.new_labeled_statement(expression, statement),
                pos
            );
            return result;
        }
        if !self.try_parse_semicolon() {
            self.parse_error_for_missing_semicolon_after(&expression);
        }
        let result = finish_node!(self, self.factory.new_expression_statement(expression), pos);
        result
    }

    pub fn parse_variable_statement(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let declaration_list = self.parse_variable_declaration_list(false);
        self.parse_semicolon();
        let result = finish_node!(
            self,
            self.factory
                .new_variable_statement(modifiers, declaration_list),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_variable_declaration_list(
        &mut self,
        in_for_statement_initializer: bool,
    ) -> ast::Node {
        let pos = self.node_pos();
        let flags = match self.token {
            ast::Kind::VarKeyword => ast::NodeFlags::NONE,
            ast::Kind::LetKeyword => ast::NodeFlags::LET,
            ast::Kind::ConstKeyword => ast::NodeFlags::CONST,
            ast::Kind::UsingKeyword => ast::NodeFlags::USING,
            ast::Kind::AwaitKeyword => {
                if self.is_await_using_declaration() {
                    self.next_token();
                    ast::NodeFlags::AWAIT_USING
                } else {
                    panic!("Unhandled case in parseVariableDeclarationList")
                }
            }
            _ => panic!("Unhandled case in parseVariableDeclarationList"),
        };
        self.next_token();
        let declarations = if self.token == ast::Kind::OfKeyword
            && self.look_ahead(Parser::next_is_identifier_and_close_paren)
        {
            self.create_missing_list()
        } else {
            let save_context_flags = self.context_flags;
            self.set_context_flags(
                ast::NodeFlags::DISALLOW_IN_CONTEXT,
                in_for_statement_initializer,
            );
            let declarations = if in_for_statement_initializer {
                self.parse_delimited_list(
                    ParsingContext::PCVariableDeclarations,
                    Parser::parse_variable_declaration,
                )
            } else {
                self.parse_delimited_list(
                    ParsingContext::PCVariableDeclarations,
                    Parser::parse_variable_declaration_allow_exclamation,
                )
            };
            self.context_flags = save_context_flags;
            declarations
        };
        finish_node!(
            self,
            self.factory
                .new_variable_declaration_list(declarations, flags),
            pos
        )
    }

    pub fn next_is_identifier_and_close_paren(&mut self) -> bool {
        self.next_token_is_identifier() && self.next_token() == ast::Kind::CloseParenToken
    }

    pub fn next_token_is_identifier(&mut self) -> bool {
        self.next_token();
        self.is_identifier()
    }

    pub fn parse_variable_declaration(&mut self) -> ast::Node {
        self.parse_variable_declaration_worker(false)
    }

    pub fn parse_variable_declaration_allow_exclamation(&mut self) -> ast::Node {
        self.parse_variable_declaration_worker(true)
    }

    pub fn parse_variable_declaration_worker(&mut self, allow_exclamation: bool) -> ast::Node {
        let pos = self.node_pos();
        let name = self.parse_identifier_or_pattern_with_diagnostic(Some(
            &diagnostics::PRIVATE_IDENTIFIERS_ARE_NOT_ALLOWED_IN_VARIABLE_DECLARATIONS,
        ));
        let exclamation_token = if allow_exclamation
            && self.factory.store().kind(name) == ast::Kind::Identifier
            && self.token == ast::Kind::ExclamationToken
            && !self.has_preceding_line_break()
        {
            Some(self.parse_token_node())
        } else {
            None
        };
        let type_node = self.parse_type_annotation();
        let initializer =
            if self.token != ast::Kind::InKeyword && self.token != ast::Kind::OfKeyword {
                self.parse_initializer()
            } else {
                None
            };
        let result = finish_node!(
            self,
            self.factory
                .new_variable_declaration(name, exclamation_token, type_node, initializer),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_identifier_or_pattern(&mut self) -> ast::Node {
        self.parse_identifier_or_pattern_with_diagnostic(None)
    }

    pub fn parse_identifier_or_pattern_with_diagnostic(
        &mut self,
        private_identifier_diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        if self.token == ast::Kind::OpenBracketToken {
            return self.parse_array_binding_pattern();
        }
        if self.token == ast::Kind::OpenBraceToken {
            return self.parse_object_binding_pattern();
        }
        self.parse_binding_identifier_with_diagnostic(private_identifier_diagnostic_message)
    }

    pub fn parse_array_binding_pattern(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenBracketToken);
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::DISALLOW_IN_CONTEXT, false);
        let elements = self.parse_delimited_list(
            ParsingContext::PCArrayBindingElements,
            Parser::parse_array_binding_element,
        );
        self.context_flags = save_context_flags;
        self.parse_expected(ast::Kind::CloseBracketToken);
        finish_node!(
            self,
            self.factory
                .new_binding_pattern(ast::Kind::ArrayBindingPattern, elements),
            pos
        )
    }

    pub fn parse_array_binding_element(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let (dot_dot_dot_token, name, initializer) = if self.token != ast::Kind::CommaToken {
            (
                self.parse_optional_token(ast::Kind::DotDotDotToken),
                Some(self.parse_identifier_or_pattern()),
                self.parse_initializer(),
            )
        } else {
            (None, None, None)
        };
        finish_node!(
            self,
            self.factory
                .new_binding_element(dot_dot_dot_token, None, name, initializer),
            pos
        )
    }

    pub fn parse_object_binding_pattern(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenBraceToken);
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::DISALLOW_IN_CONTEXT, false);
        let elements = self.parse_delimited_list(
            ParsingContext::PCObjectBindingElements,
            Parser::parse_object_binding_element,
        );
        self.context_flags = save_context_flags;
        self.parse_expected(ast::Kind::CloseBraceToken);
        finish_node!(
            self,
            self.factory
                .new_binding_pattern(ast::Kind::ObjectBindingPattern, elements),
            pos
        )
    }

    pub fn parse_object_binding_element(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let dot_dot_dot_token = self.parse_optional_token(ast::Kind::DotDotDotToken);
        let token_is_identifier = self.is_binding_identifier();
        let mut property_name = Some(self.parse_property_name());
        let name = if token_is_identifier && self.token != ast::Kind::ColonToken {
            let name = property_name.clone();
            property_name = None;
            name
        } else {
            self.parse_expected(ast::Kind::ColonToken);
            Some(self.parse_identifier_or_pattern())
        };
        let initializer = self.parse_initializer();
        finish_node!(
            self,
            self.factory
                .new_binding_element(dot_dot_dot_token, property_name, name, initializer),
            pos
        )
    }

    pub fn parse_property_name(&mut self) -> ast::Node {
        let save_has_await_identifier = self.statement_has_await_identifier;
        let prop = self.parse_property_name_worker(true);
        self.statement_has_await_identifier = save_has_await_identifier;
        prop
    }

    pub fn parse_property_name_worker(&mut self, allow_computed_property_names: bool) -> ast::Node {
        if self.token == ast::Kind::StringLiteral
            || self.token == ast::Kind::NumericLiteral
            || self.token == ast::Kind::BigIntLiteral
        {
            return self.parse_literal_expression(true);
        }
        if allow_computed_property_names && self.token == ast::Kind::OpenBracketToken {
            return self.parse_computed_property_name();
        }
        if self.token == ast::Kind::PrivateIdentifier {
            return self.parse_private_identifier();
        }
        self.parse_identifier_name()
    }

    pub fn parse_computed_property_name(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenBracketToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected(ast::Kind::CloseBracketToken);
        finish_node!(
            self,
            self.factory.new_computed_property_name(expression),
            pos
        )
    }

    pub fn parse_initializer(&mut self) -> Option<ast::Node> {
        if self.parse_optional(ast::Kind::EqualsToken) {
            return Some(self.parse_assignment_expression_or_higher());
        }
        None
    }

    pub fn parse_type_annotation(&mut self) -> Option<ast::Node> {
        if self.parse_optional(ast::Kind::ColonToken) {
            return Some(self.parse_type());
        }
        None
    }

    pub fn parse_error_for_invalid_name(
        &mut self,
        name_diagnostic: &diagnostics::Message,
        blank_diagnostic: &diagnostics::Message,
        token_if_blank_name: ast::Kind,
    ) {
        if self.token == token_if_blank_name {
            self.parse_error_at_current_token(blank_diagnostic, Vec::new());
        } else {
            let token_value = self
                .scanner
                .as_ref()
                .expect("scanner")
                .token_value()
                .to_string();
            self.parse_error_at_current_token(
                name_diagnostic,
                vec![diagnostics::Any::from(token_value)],
            );
        }
    }

    pub fn can_parse_semicolon(&self) -> bool {
        self.token == ast::Kind::SemicolonToken
            || self.token == ast::Kind::CloseBraceToken
            || self.token == ast::Kind::EndOfFile
            || self.has_preceding_line_break()
    }

    pub fn try_parse_semicolon(&mut self) -> bool {
        if !self.can_parse_semicolon() {
            return false;
        }
        if self.token == ast::Kind::SemicolonToken {
            self.next_token();
        }
        true
    }

    pub fn parse_semicolon(&mut self) -> bool {
        self.try_parse_semicolon() || self.parse_expected(ast::Kind::SemicolonToken)
    }

    pub fn is_literal_property_name(&self) -> bool {
        token_is_identifier_or_keyword(self.token)
            || self.token == ast::Kind::StringLiteral
            || self.token == ast::Kind::NumericLiteral
            || self.token == ast::Kind::BigIntLiteral
    }

    pub fn is_start_of_statement(&mut self) -> bool {
        match self.token {
            ast::Kind::AtToken
            | ast::Kind::SemicolonToken
            | ast::Kind::OpenBraceToken
            | ast::Kind::VarKeyword
            | ast::Kind::LetKeyword
            | ast::Kind::UsingKeyword
            | ast::Kind::FunctionKeyword
            | ast::Kind::ClassKeyword
            | ast::Kind::EnumKeyword
            | ast::Kind::IfKeyword
            | ast::Kind::DoKeyword
            | ast::Kind::WhileKeyword
            | ast::Kind::ForKeyword
            | ast::Kind::ContinueKeyword
            | ast::Kind::BreakKeyword
            | ast::Kind::ReturnKeyword
            | ast::Kind::WithKeyword
            | ast::Kind::SwitchKeyword
            | ast::Kind::ThrowKeyword
            | ast::Kind::TryKeyword
            | ast::Kind::DebuggerKeyword
            | ast::Kind::CatchKeyword
            | ast::Kind::FinallyKeyword => true,
            ast::Kind::ImportKeyword => {
                self.is_start_of_declaration()
                    || self.is_next_token_open_paren_or_less_than_or_dot()
            }
            ast::Kind::ConstKeyword | ast::Kind::ExportKeyword => self.is_start_of_declaration(),
            ast::Kind::AsyncKeyword
            | ast::Kind::DeclareKeyword
            | ast::Kind::InterfaceKeyword
            | ast::Kind::ModuleKeyword
            | ast::Kind::NamespaceKeyword
            | ast::Kind::TypeKeyword
            | ast::Kind::GlobalKeyword
            | ast::Kind::DeferKeyword => true,
            ast::Kind::AccessorKeyword
            | ast::Kind::PublicKeyword
            | ast::Kind::PrivateKeyword
            | ast::Kind::ProtectedKeyword
            | ast::Kind::StaticKeyword
            | ast::Kind::ReadonlyKeyword => {
                self.is_start_of_declaration()
                    || !self.look_ahead(Parser::next_token_is_identifier_or_keyword_on_same_line)
            }
            _ => self.is_start_of_expression(),
        }
    }

    pub fn is_start_of_declaration(&mut self) -> bool {
        self.look_ahead(Parser::scan_start_of_declaration)
    }

    pub fn scan_start_of_declaration(&mut self) -> bool {
        loop {
            match self.token {
                ast::Kind::VarKeyword
                | ast::Kind::LetKeyword
                | ast::Kind::ConstKeyword
                | ast::Kind::FunctionKeyword
                | ast::Kind::ClassKeyword
                | ast::Kind::EnumKeyword => return true,
                ast::Kind::UsingKeyword => return self.is_using_declaration(),
                ast::Kind::AwaitKeyword => return self.is_await_using_declaration(),
                ast::Kind::InterfaceKeyword | ast::Kind::TypeKeyword | ast::Kind::DeferKeyword => {
                    return self.next_token_is_identifier_on_same_line();
                }
                ast::Kind::ModuleKeyword | ast::Kind::NamespaceKeyword => {
                    return self.next_token_is_identifier_or_string_literal_on_same_line();
                }
                ast::Kind::AbstractKeyword
                | ast::Kind::AccessorKeyword
                | ast::Kind::AsyncKeyword
                | ast::Kind::DeclareKeyword
                | ast::Kind::PrivateKeyword
                | ast::Kind::ProtectedKeyword
                | ast::Kind::PublicKeyword
                | ast::Kind::ReadonlyKeyword => {
                    let previous_token = self.token;
                    self.next_token();
                    if self.has_preceding_line_break() {
                        return false;
                    }
                    if previous_token == ast::Kind::DeclareKeyword
                        && self.token == ast::Kind::TypeKeyword
                    {
                        return true;
                    }
                    continue;
                }
                ast::Kind::GlobalKeyword => {
                    self.next_token();
                    return self.token == ast::Kind::OpenBraceToken
                        || self.token == ast::Kind::Identifier
                        || self.token == ast::Kind::ExportKeyword;
                }
                ast::Kind::ImportKeyword => {
                    self.next_token();
                    return self.token == ast::Kind::DeferKeyword
                        || self.token == ast::Kind::StringLiteral
                        || self.token == ast::Kind::AsteriskToken
                        || self.token == ast::Kind::OpenBraceToken
                        || token_is_identifier_or_keyword(self.token);
                }
                ast::Kind::ExportKeyword => {
                    self.next_token();
                    if self.token == ast::Kind::EqualsToken
                        || self.token == ast::Kind::AsteriskToken
                        || self.token == ast::Kind::OpenBraceToken
                        || self.token == ast::Kind::DefaultKeyword
                        || self.token == ast::Kind::AsKeyword
                        || self.token == ast::Kind::AtToken
                    {
                        return true;
                    }
                    if self.token == ast::Kind::TypeKeyword {
                        self.next_token();
                        return self.token == ast::Kind::AsteriskToken
                            || self.token == ast::Kind::OpenBraceToken
                            || self.is_identifier() && !self.has_preceding_line_break();
                    }
                    continue;
                }
                ast::Kind::StaticKeyword => {
                    self.next_token();
                    continue;
                }
                _ => return false,
            }
        }
    }

    pub fn is_start_of_expression(&mut self) -> bool {
        if self.is_start_of_left_hand_side_expression() {
            return true;
        }
        match self.token {
            ast::Kind::PlusToken
            | ast::Kind::MinusToken
            | ast::Kind::TildeToken
            | ast::Kind::ExclamationToken
            | ast::Kind::DeleteKeyword
            | ast::Kind::TypeOfKeyword
            | ast::Kind::VoidKeyword
            | ast::Kind::PlusPlusToken
            | ast::Kind::MinusMinusToken
            | ast::Kind::LessThanToken
            | ast::Kind::AwaitKeyword
            | ast::Kind::YieldKeyword
            | ast::Kind::PrivateIdentifier
            | ast::Kind::AtToken => return true,
            _ => {}
        }
        if self.is_binary_operator() {
            return true;
        }
        self.is_identifier()
    }

    pub fn is_start_of_left_hand_side_expression(&mut self) -> bool {
        match self.token {
            ast::Kind::ThisKeyword
            | ast::Kind::SuperKeyword
            | ast::Kind::NullKeyword
            | ast::Kind::TrueKeyword
            | ast::Kind::FalseKeyword
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::StringLiteral
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::TemplateHead
            | ast::Kind::OpenParenToken
            | ast::Kind::OpenBracketToken
            | ast::Kind::OpenBraceToken
            | ast::Kind::FunctionKeyword
            | ast::Kind::ClassKeyword
            | ast::Kind::NewKeyword
            | ast::Kind::SlashToken
            | ast::Kind::SlashEqualsToken
            | ast::Kind::Identifier => return true,
            ast::Kind::ImportKeyword => return self.is_next_token_open_paren_or_less_than_or_dot(),
            _ => {}
        }
        self.is_identifier()
    }

    pub fn is_start_of_type(&mut self, in_start_of_parameter: bool) -> bool {
        match self.token {
            ast::Kind::AnyKeyword
            | ast::Kind::UnknownKeyword
            | ast::Kind::StringKeyword
            | ast::Kind::NumberKeyword
            | ast::Kind::BigIntKeyword
            | ast::Kind::BooleanKeyword
            | ast::Kind::ReadonlyKeyword
            | ast::Kind::SymbolKeyword
            | ast::Kind::UniqueKeyword
            | ast::Kind::VoidKeyword
            | ast::Kind::UndefinedKeyword
            | ast::Kind::NullKeyword
            | ast::Kind::ThisKeyword
            | ast::Kind::TypeOfKeyword
            | ast::Kind::NeverKeyword
            | ast::Kind::OpenBraceToken
            | ast::Kind::OpenBracketToken
            | ast::Kind::LessThanToken
            | ast::Kind::BarToken
            | ast::Kind::AmpersandToken
            | ast::Kind::NewKeyword
            | ast::Kind::StringLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::TrueKeyword
            | ast::Kind::FalseKeyword
            | ast::Kind::ObjectKeyword
            | ast::Kind::AsteriskToken
            | ast::Kind::QuestionToken
            | ast::Kind::ExclamationToken
            | ast::Kind::DotDotDotToken
            | ast::Kind::InferKeyword
            | ast::Kind::ImportKeyword
            | ast::Kind::AssertsKeyword
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::TemplateHead => return true,
            ast::Kind::FunctionKeyword => return !in_start_of_parameter,
            ast::Kind::MinusToken => {
                return !in_start_of_parameter
                    && self.look_ahead(Parser::next_token_is_numeric_or_big_int_literal);
            }
            ast::Kind::OpenParenToken => {
                return !in_start_of_parameter
                    && self.look_ahead(Parser::next_is_parenthesized_or_function_type);
            }
            _ => {}
        }
        self.is_identifier()
    }

    pub fn next_token_is_numeric_or_big_int_literal(&mut self) -> bool {
        self.next_token();
        self.token == ast::Kind::NumericLiteral || self.token == ast::Kind::BigIntLiteral
    }

    pub fn next_is_parenthesized_or_function_type(&mut self) -> bool {
        self.next_token();
        self.token == ast::Kind::CloseParenToken
            || self.is_start_of_parameter()
            || self.is_start_of_type(false)
    }

    pub fn is_start_of_parameter(&mut self) -> bool {
        self.token == ast::Kind::DotDotDotToken
            || self.is_binding_identifier_or_private_identifier_or_pattern()
            || ast::is_modifier_kind(self.token)
            || self.token == ast::Kind::AtToken
            || self.is_start_of_type(true)
    }

    pub fn is_binding_identifier_or_private_identifier_or_pattern(&mut self) -> bool {
        self.token == ast::Kind::OpenBraceToken
            || self.token == ast::Kind::OpenBracketToken
            || self.token == ast::Kind::PrivateIdentifier
            || self.is_binding_identifier()
    }

    pub fn is_next_token_open_paren_or_less_than_or_dot(&mut self) -> bool {
        self.look_ahead(Parser::next_token_is_open_paren_or_less_than_or_dot)
    }

    pub fn next_token_is_open_paren_or_less_than_or_dot(&mut self) -> bool {
        matches!(
            self.next_token(),
            ast::Kind::OpenParenToken | ast::Kind::LessThanToken | ast::Kind::DotToken
        )
    }

    pub fn next_token_is_open_paren(&mut self) -> bool {
        self.next_token() == ast::Kind::OpenParenToken
    }

    pub fn next_token_is_dot(&mut self) -> bool {
        self.next_token() == ast::Kind::DotToken
    }

    pub fn next_token_is_open_brace(&mut self) -> bool {
        self.next_token() == ast::Kind::OpenBraceToken
    }

    pub fn next_token_is_open_paren_or_less_than(&mut self) -> bool {
        matches!(
            self.next_token(),
            ast::Kind::OpenParenToken | ast::Kind::LessThanToken
        )
    }

    pub fn next_token_is_identifier_or_keyword(&mut self) -> bool {
        self.next_token();
        token_is_identifier_or_keyword(self.token)
    }

    pub fn next_token_is_identifier_or_keyword_on_same_line(&mut self) -> bool {
        self.next_token();
        token_is_identifier_or_keyword(self.token) && !self.has_preceding_line_break()
    }

    pub fn next_token_is_identifier_or_keyword_or_literal_on_same_line(&mut self) -> bool {
        self.next_token();
        (token_is_identifier_or_keyword(self.token)
            || self.token == ast::Kind::NumericLiteral
            || self.token == ast::Kind::BigIntLiteral
            || self.token == ast::Kind::StringLiteral)
            && !self.has_preceding_line_break()
    }

    pub fn next_token_is_identifier_or_keyword_or_greater_than(&mut self) -> bool {
        self.next_token();
        token_is_identifier_or_keyword(self.token) || self.token == ast::Kind::GreaterThanToken
    }

    pub fn next_token_is_identifier_on_same_line(&mut self) -> bool {
        self.next_token();
        self.is_identifier() && !self.has_preceding_line_break()
    }

    pub fn next_token_is_identifier_or_string_literal_on_same_line(&mut self) -> bool {
        self.next_token();
        (self.is_identifier() || self.token == ast::Kind::StringLiteral)
            && !self.has_preceding_line_break()
    }

    pub fn is_identifier(&self) -> bool {
        if self.token == ast::Kind::Identifier {
            return true;
        }
        if self.token == ast::Kind::YieldKeyword && self.in_yield_context()
            || self.token == ast::Kind::AwaitKeyword && self.in_await_context()
        {
            return false;
        }
        self.token > ast::Kind::LastReservedWord
    }

    pub fn is_binding_identifier(&self) -> bool {
        self.token == ast::Kind::Identifier || self.token > ast::Kind::LastReservedWord
    }

    pub fn is_import_attribute_name(&self) -> bool {
        token_is_identifier_or_keyword(self.token) || self.token == ast::Kind::StringLiteral
    }

    pub fn is_binary_operator(&self) -> bool {
        if self.in_disallow_in_context() && self.token == ast::Kind::InKeyword {
            return false;
        }
        ast::get_binary_operator_precedence(self.token) != ast::OPERATOR_PRECEDENCE_INVALID
    }

    pub fn is_valid_heritage_clause_object_literal(&mut self) -> bool {
        self.look_ahead(Parser::next_is_valid_heritage_clause_object_literal)
    }

    pub fn next_is_valid_heritage_clause_object_literal(&mut self) -> bool {
        if self.next_token() == ast::Kind::CloseBraceToken {
            let next = self.next_token();
            return next == ast::Kind::CommaToken
                || next == ast::Kind::OpenBraceToken
                || next == ast::Kind::ExtendsKeyword
                || next == ast::Kind::ImplementsKeyword;
        }
        true
    }

    pub fn is_heritage_clause(&self) -> bool {
        self.token == ast::Kind::ExtendsKeyword || self.token == ast::Kind::ImplementsKeyword
    }

    pub fn is_heritage_clause_extends_or_implements_keyword(&mut self) -> bool {
        self.is_heritage_clause() && self.look_ahead(Parser::next_is_start_of_expression)
    }

    pub fn next_is_start_of_expression(&mut self) -> bool {
        self.next_token();
        self.is_start_of_expression()
    }

    pub fn is_using_declaration(&mut self) -> bool {
        self.look_ahead(|p| {
            p.next_token_is_binding_identifier_or_start_of_destructuring_on_same_line(false)
        })
    }

    pub fn next_token_is_equals_or_semicolon_or_colon_token(&mut self) -> bool {
        self.next_token();
        self.token == ast::Kind::EqualsToken
            || self.token == ast::Kind::SemicolonToken
            || self.token == ast::Kind::ColonToken
    }

    pub fn next_token_is_binding_identifier_or_start_of_destructuring_on_same_line(
        &mut self,
        disallow_of: bool,
    ) -> bool {
        self.next_token();
        if disallow_of && self.token == ast::Kind::OfKeyword {
            return self.look_ahead(Parser::next_token_is_equals_or_semicolon_or_colon_token);
        }
        self.is_binding_identifier()
            || self.token == ast::Kind::OpenBraceToken && !self.has_preceding_line_break()
    }

    pub fn next_token_is_binding_identifier_or_start_of_destructuring_on_same_line_disallow_of(
        &mut self,
    ) -> bool {
        self.next_token_is_binding_identifier_or_start_of_destructuring_on_same_line(true)
    }

    pub fn is_await_using_declaration(&mut self) -> bool {
        self.look_ahead(Parser::next_is_using_keyword_then_binding_identifier_or_start_of_object_destructuring_on_same_line)
    }

    pub fn next_is_using_keyword_then_binding_identifier_or_start_of_object_destructuring_on_same_line(
        &mut self,
    ) -> bool {
        self.next_token() == ast::Kind::UsingKeyword
            && self.next_token_is_binding_identifier_or_start_of_destructuring_on_same_line(false)
    }

    pub fn next_token_is_token_string_literal(&mut self) -> bool {
        self.next_token() == ast::Kind::StringLiteral
    }

    pub fn set_context_flags(&mut self, flags: ast::NodeFlags, value: bool) {
        if value {
            self.context_flags |= flags;
        } else {
            self.context_flags &= !flags;
        }
    }

    pub fn in_yield_context(&self) -> bool {
        self.context_flags & ast::NodeFlags::YIELD_CONTEXT != ast::NodeFlags::NONE
    }

    pub fn in_disallow_in_context(&self) -> bool {
        self.context_flags & ast::NodeFlags::DISALLOW_IN_CONTEXT != ast::NodeFlags::NONE
    }

    pub fn in_disallow_conditional_types_context(&self) -> bool {
        self.context_flags & ast::NodeFlags::DISALLOW_CONDITIONAL_TYPES_CONTEXT
            != ast::NodeFlags::NONE
    }

    pub fn in_decorator_context(&self) -> bool {
        self.context_flags & ast::NodeFlags::DECORATOR_CONTEXT != ast::NodeFlags::NONE
    }

    pub fn in_await_context(&self) -> bool {
        self.context_flags & ast::NodeFlags::AWAIT_CONTEXT != ast::NodeFlags::NONE
    }

    pub fn skip_range_trivia(&self, text_range: core::TextRange) -> core::TextRange {
        core::TextRange::new(
            scanner::skip_trivia(&self.source_text, text_range.pos() as usize) as i32,
            text_range.end(),
        )
    }

    pub fn new_identifier(&mut self, text: String) -> ast::Node {
        self.identifier_count += 1;
        if text == "await" {
            self.statement_has_await_identifier = true;
        }
        self.factory.new_identifier(text)
    }

    pub fn create_missing_identifier(&mut self) -> ast::Node {
        finish_node!(self, self.new_identifier(String::new()), self.node_pos())
    }

    pub fn parse_private_identifier(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let interned = self.intern_current_token_value();
        self.next_token();
        finish_node!(self, self.factory.new_private_identifier(interned), pos)
    }

    pub fn rescan_less_than_token(&mut self) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .rescan_less_than_token();
        self.token
    }

    pub fn rescan_greater_than_token(&mut self) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .rescan_greater_than_token();
        self.token
    }

    pub fn rescan_slash_token(&mut self) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .rescan_slash_token(false);
        self.token
    }

    pub fn rescan_template_token(&mut self, is_tagged_template: bool) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .rescan_template_token(is_tagged_template);
        self.token
    }

    pub fn parse_literal_expression(&mut self, intern: bool) -> ast::Node {
        let pos = self.node_pos();
        let text = if intern {
            self.intern_current_token_value()
        } else {
            self.scanner
                .as_ref()
                .expect("scanner")
                .token_value()
                .to_string()
        };
        let token_flags = self.scanner.as_ref().expect("scanner").token_flags();
        let result = match self.token {
            ast::Kind::StringLiteral => self.factory.new_string_literal(text, token_flags),
            ast::Kind::NumericLiteral => self.factory.new_numeric_literal(text, token_flags),
            ast::Kind::BigIntLiteral => self.factory.new_big_int_literal(text, token_flags),
            ast::Kind::RegularExpressionLiteral => self
                .factory
                .new_regular_expression_literal(text, token_flags),
            ast::Kind::NoSubstitutionTemplateLiteral => self
                .factory
                .new_no_substitution_template_literal(text, token_flags),
            _ => panic!("Unhandled case in parseLiteralExpression"),
        };
        self.next_token();
        self.finish_node(result, pos)
    }

    pub fn parse_identifier_name_error_on_unicode_escape_sequence(&mut self) -> ast::Node {
        if self.scanner.as_ref().expect("scanner").has_unicode_escape()
            || self
                .scanner
                .as_ref()
                .expect("scanner")
                .has_extended_unicode_escape()
        {
            self.parse_error_at_current_token(
                &diagnostics::UNICODE_ESCAPE_SEQUENCE_CANNOT_APPEAR_HERE,
                Vec::new(),
            );
        }
        self.create_identifier(token_is_identifier_or_keyword(self.token))
    }

    pub fn parse_binding_identifier(&mut self) -> ast::Node {
        self.parse_binding_identifier_with_diagnostic(None)
    }

    pub fn parse_binding_identifier_with_diagnostic(
        &mut self,
        private_identifier_diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        let save_has_await_identifier = self.statement_has_await_identifier;
        let id = self.create_identifier_with_diagnostic(
            self.is_binding_identifier(),
            None,
            private_identifier_diagnostic_message,
        );
        self.statement_has_await_identifier = save_has_await_identifier;
        id
    }

    pub fn parse_identifier_name(&mut self) -> ast::Node {
        self.parse_identifier_name_with_diagnostic(None)
    }

    pub fn parse_identifier_name_with_diagnostic(
        &mut self,
        diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        self.create_identifier_with_diagnostic(
            token_is_identifier_or_keyword(self.token),
            diagnostic_message,
            None,
        )
    }

    pub fn parse_identifier(&mut self) -> ast::Node {
        self.parse_identifier_with_diagnostic(None, None)
    }

    pub fn parse_identifier_with_diagnostic(
        &mut self,
        diagnostic_message: Option<&diagnostics::Message>,
        private_identifier_diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        self.create_identifier_with_diagnostic(
            self.is_identifier(),
            diagnostic_message,
            private_identifier_diagnostic_message,
        )
    }

    pub fn create_identifier(&mut self, is_identifier: bool) -> ast::Node {
        self.create_identifier_with_diagnostic(is_identifier, None, None)
    }

    pub fn create_identifier_with_diagnostic(
        &mut self,
        is_identifier: bool,
        diagnostic_message: Option<&diagnostics::Message>,
        private_identifier_diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        if is_identifier {
            let pos = self.node_pos();
            let interned = self.intern_current_token_value();
            self.next_token_without_check();
            return finish_node!(self, self.new_identifier(interned), pos);
        }
        if self.token == ast::Kind::PrivateIdentifier {
            if let Some(message) = private_identifier_diagnostic_message {
                self.parse_error_at_current_token(message, Vec::new());
            } else {
                self.parse_error_at_current_token(
                    &diagnostics::PRIVATE_IDENTIFIERS_ARE_NOT_ALLOWED_OUTSIDE_CLASS_BODIES,
                    Vec::new(),
                );
            }
            return self.create_identifier(true);
        }
        let report_at_current_position = self.token == ast::Kind::EndOfFile;
        if let Some(message) = diagnostic_message {
            if report_at_current_position {
                let pos = self.scanner.as_ref().expect("scanner").token_full_start();
                self.parse_error_at(pos, pos, message, Vec::new());
            } else {
                self.parse_error_at_current_token(message, Vec::new());
            }
        } else if is_reserved_word(self.token) {
            let token_text = self
                .scanner
                .as_ref()
                .expect("scanner")
                .token_text()
                .to_string();
            if report_at_current_position {
                let pos = self.scanner.as_ref().expect("scanner").token_full_start();
                self.parse_error_at(
                    pos,
                    pos,
                    &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_THAT_CANNOT_BE_USED_HERE,
                    vec![diagnostics::Any::from(token_text)],
                );
            } else {
                self.parse_error_at_current_token(
                    &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_THAT_CANNOT_BE_USED_HERE,
                    vec![diagnostics::Any::from(token_text)],
                );
            }
        } else if report_at_current_position {
            let pos = self.scanner.as_ref().expect("scanner").token_full_start();
            self.parse_error_at(pos, pos, &diagnostics::IDENTIFIER_EXPECTED, Vec::new());
        } else {
            self.parse_error_at_current_token(&diagnostics::IDENTIFIER_EXPECTED, Vec::new());
        }
        self.create_missing_identifier()
    }

    pub fn intern_identifier(&mut self, text: String) -> String {
        if let Some(identifier) = self.identifiers.get(text.as_str()) {
            return identifier.clone();
        }
        self.identifiers.insert(text.clone(), text.clone());
        text
    }

    pub fn intern_identifier_str(&mut self, text: &str) -> String {
        if let Some(identifier) = self.identifiers.get(text) {
            return identifier.clone();
        }
        let text = text.to_string();
        self.identifiers.insert(text.clone(), text.clone());
        text
    }

    fn intern_current_token_value(&mut self) -> String {
        let scanner = self.scanner.as_ref().expect("scanner");
        if let Some(identifier) = self.identifiers.get(scanner.token_value()) {
            return identifier.clone();
        }
        let text = scanner.token_value().to_string();
        self.identifiers.insert(text.clone(), text.clone());
        text
    }

    pub fn finish_node_with_end(&mut self, node: ast::Node, pos: i32, end: i32) -> ast::Node {
        let loc = core::TextRange::new(pos, end);
        self.factory
            .finish_parsed_node_header(node, loc, self.context_flags, self.has_parse_error);
        self.has_parse_error = false;
        self.override_parent_in_immediate_children(&node);
        node
    }

    pub fn override_parent_in_immediate_children(&mut self, node: &ast::Node) {
        self.factory.adopt_parsed_children(*node);
        self.current_parent = None;
    }

    pub fn next_token_is_slash(&mut self) -> bool {
        self.next_token() == ast::Kind::SlashToken
    }

    pub fn scan_type_member_start(&mut self) -> bool {
        if matches!(
            self.token,
            ast::Kind::OpenParenToken
                | ast::Kind::LessThanToken
                | ast::Kind::GetKeyword
                | ast::Kind::SetKeyword
        ) {
            return true;
        }
        let mut id_token = false;
        while ast::is_modifier_kind(self.token) {
            id_token = true;
            self.next_token();
        }
        if self.token == ast::Kind::OpenBracketToken {
            return true;
        }
        if self.is_literal_property_name() {
            id_token = true;
            self.next_token();
        }
        if id_token {
            return matches!(
                self.token,
                ast::Kind::OpenParenToken
                    | ast::Kind::LessThanToken
                    | ast::Kind::QuestionToken
                    | ast::Kind::ColonToken
                    | ast::Kind::CommaToken
            ) || self.can_parse_semicolon();
        }
        false
    }

    pub fn scan_class_member_start(&mut self) -> bool {
        let mut id_token = ast::Kind::Unknown;
        if self.token == ast::Kind::AtToken {
            return true;
        }
        while ast::is_modifier_kind(self.token) {
            id_token = self.token;
            if ast::is_class_member_modifier(id_token) {
                return true;
            }
            self.next_token();
        }
        if self.token == ast::Kind::AsteriskToken {
            return true;
        }
        if self.is_literal_property_name() {
            id_token = self.token;
            self.next_token();
        }
        if self.token == ast::Kind::OpenBracketToken {
            return true;
        }
        if id_token != ast::Kind::Unknown {
            if !ast::is_keyword(id_token)
                || id_token == ast::Kind::SetKeyword
                || id_token == ast::Kind::GetKeyword
            {
                return true;
            }
            return matches!(
                self.token,
                ast::Kind::OpenParenToken
                    | ast::Kind::LessThanToken
                    | ast::Kind::ExclamationToken
                    | ast::Kind::ColonToken
                    | ast::Kind::EqualsToken
                    | ast::Kind::QuestionToken
            ) || self.can_parse_semicolon();
        }
        false
    }

    pub fn parse_function_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_expected(ast::Kind::FunctionKeyword);
        let asterisk_token = self.parse_optional_token(ast::Kind::AsteriskToken);
        let name = if modifiers.as_ref().is_none_or(|m| {
            !self
                .factory
                .parsed_modifier_flags(*m)
                .contains(ast::ModifierFlags::DEFAULT)
        }) || self.is_binding_identifier()
        {
            Some(self.parse_binding_identifier())
        } else {
            None
        };
        let signature_flags =
            if asterisk_token.is_some() {
                crate::ParseFlags::YIELD
            } else {
                crate::ParseFlags::NONE
            } | if modifier_list_has_async(self.factory.store(), modifiers.as_ref()) {
                crate::ParseFlags::AWAIT
            } else {
                crate::ParseFlags::NONE
            };
        let type_parameters = self.parse_type_parameters();
        let save_context_flags = self.context_flags;
        if modifiers.as_ref().is_some_and(|m| {
            self.factory
                .store()
                .parser_access()
                .modifier_list_nodes(*m)
                .into_iter()
                .any(|modifier| is_export_modifier(self.factory.store(), &modifier))
        }) {
            self.set_context_flags(ast::NodeFlags::AWAIT_CONTEXT, true);
        }
        let parameters = self.parse_parameters(signature_flags);
        let return_type = self.parse_return_type(ast::Kind::ColonToken, false);
        let body = self
            .parse_function_block_or_semicolon(signature_flags, Some(&diagnostics::X_OR_EXPECTED));
        self.context_flags = save_context_flags;
        let result = finish_node!(
            self,
            self.factory.new_function_declaration(
                modifiers,
                asterisk_token,
                name,
                type_parameters,
                parameters,
                return_type,
                None,
                body
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_class_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_class_declaration_or_expression(pos, modifiers, ast::Kind::ClassDeclaration)
    }

    pub fn parse_class_expression(&mut self) -> ast::Node {
        self.parse_class_declaration_or_expression(
            self.node_pos(),
            None,
            ast::Kind::ClassExpression,
        )
    }

    pub fn parse_class_declaration_or_expression(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
        kind: ast::Kind,
    ) -> ast::Node {
        let save_context_flags = self.context_flags;
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.parse_expected(ast::Kind::ClassKeyword);
        let name = self.parse_name_of_class_declaration_or_expression();
        let type_parameters = self.parse_type_parameters();
        if modifiers.as_ref().is_some_and(|m| {
            self.factory
                .store()
                .parser_access()
                .modifier_list_nodes(*m)
                .into_iter()
                .any(|modifier| is_export_modifier(self.factory.store(), &modifier))
        }) {
            self.set_context_flags(ast::NodeFlags::AWAIT_CONTEXT, true);
        }
        let heritage_clauses = self.parse_heritage_clauses();
        let members = if self.parse_expected(ast::Kind::OpenBraceToken) {
            let members =
                self.parse_list(ParsingContext::PCClassMembers, Parser::parse_class_element);
            self.parse_expected(ast::Kind::CloseBraceToken);
            members
        } else {
            self.create_missing_list()
        };
        self.context_flags = save_context_flags;
        if modifiers.as_ref().is_some_and(|m| {
            self.factory
                .parsed_modifier_flags(*m)
                .contains(ast::ModifierFlags::AMBIENT)
        }) {
            self.statement_has_await_identifier = save_has_await_identifier;
        }
        let result = if kind == ast::Kind::ClassDeclaration {
            self.factory.new_class_declaration(
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
            )
        } else {
            self.factory.new_class_expression(
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members,
            )
        };
        let result = self.finish_node(result, pos);
        if self
            .factory
            .store()
            .flags(result)
            .contains(ast::NodeFlags::JAVASCRIPT_FILE)
        {
            self.check_js_syntax(result.clone());
            let heritage_clauses = self.factory.store().heritage_clauses(result);
            if let Some(clauses) = heritage_clauses {
                let clauses: Vec<_> = clauses.iter().collect();
                for clause in clauses {
                    let (is_extends, types) = {
                        (
                            self.factory.store().token(clause) == Some(ast::Kind::ExtendsKeyword),
                            self.factory
                                .store()
                                .types(clause)
                                .expect("HeritageClause.types"),
                        )
                    };
                    if is_extends {
                        let exprs: Vec<_> = types.iter().collect();
                        for expr in exprs {
                            self.check_js_syntax(expr);
                        }
                    }
                }
            }
        }
        result
    }

    pub fn parse_name_of_class_declaration_or_expression(&mut self) -> Option<ast::Node> {
        if self.is_binding_identifier() && !self.is_implements_clause() {
            let save_has_await_identifier = self.statement_has_await_identifier;
            let id = self.create_identifier(self.is_binding_identifier());
            self.statement_has_await_identifier = save_has_await_identifier;
            return Some(id);
        }
        None
    }

    pub fn is_implements_clause(&mut self) -> bool {
        self.token == ast::Kind::ImplementsKeyword
            && self.look_ahead(Parser::next_token_is_identifier_or_keyword)
    }

    pub fn parse_heritage_clauses(&mut self) -> Option<ast::NodeList> {
        if self.is_heritage_clause() {
            return Some(self.parse_list(
                ParsingContext::PCHeritageClauses,
                Parser::parse_heritage_clause,
            ));
        }
        None
    }

    pub fn parse_heritage_clause(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let kind = self.token;
        self.next_token();
        let types = self.parse_delimited_list(
            ParsingContext::PCHeritageClauseElement,
            Parser::parse_expression_with_type_arguments,
        );
        let heritage_clause =
            finish_node!(self, self.factory.new_heritage_clause(kind, types), pos);
        self.check_js_syntax(heritage_clause)
    }

    pub fn parse_expression_with_type_arguments(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let expression = self.parse_left_hand_side_expression_or_higher();
        if ast::is_expression_with_type_arguments(self.factory.store(), expression) {
            return expression;
        }
        let type_arguments = self.parse_type_arguments();
        finish_node!(
            self,
            self.factory
                .new_expression_with_type_arguments(expression, type_arguments),
            pos
        )
    }

    pub fn parse_class_element(&mut self) -> ast::Node {
        let pos = self.node_pos();
        if self.token == ast::Kind::SemicolonToken {
            self.next_token();
            let result = finish_node!(self, self.factory.new_semicolon_class_element(), pos);
            return result;
        }
        let modifiers = self.parse_modifiers_ex(true, true, true);
        if self.token == ast::Kind::StaticKeyword
            && self.look_ahead(Parser::next_token_is_open_brace)
        {
            return self.parse_class_static_block_declaration(pos, modifiers);
        }
        if self.parse_contextual_modifier(ast::Kind::GetKeyword) {
            return self.parse_accessor_declaration(
                pos,
                modifiers,
                ast::Kind::GetAccessor,
                crate::ParseFlags::NONE,
            );
        }
        if self.parse_contextual_modifier(ast::Kind::SetKeyword) {
            return self.parse_accessor_declaration(
                pos,
                modifiers,
                ast::Kind::SetAccessor,
                crate::ParseFlags::NONE,
            );
        }
        if self.token == ast::Kind::ConstructorKeyword
            || self.token == ast::Kind::StringLiteral
                && self.scanner.as_ref().expect("scanner").token_value() == "constructor"
                && self.look_ahead(Parser::next_token_is_open_paren)
        {
            if let Some(constructor) =
                self.try_parse_constructor_declaration(pos, modifiers.clone())
            {
                return constructor;
            }
        }
        if self.is_index_signature() {
            let index_signature = self.parse_index_signature_declaration(pos, modifiers);
            return self.check_js_syntax(index_signature);
        }
        if token_is_identifier_or_keyword(self.token)
            || matches!(
                self.token,
                ast::Kind::StringLiteral
                    | ast::Kind::NumericLiteral
                    | ast::Kind::BigIntLiteral
                    | ast::Kind::AsteriskToken
                    | ast::Kind::OpenBracketToken
            )
        {
            if modifiers.as_ref().is_some_and(|m| {
                self.factory
                    .store()
                    .parser_access()
                    .modifier_list_nodes(*m)
                    .into_iter()
                    .any(|modifier| is_declare_modifier(self.factory.store(), &modifier))
            }) {
                if let Some(modifiers) = modifiers {
                    let modifiers = self.factory.parsed_modifier_list_nodes(modifiers);
                    for modifier in modifiers {
                        self.factory.mark_parsed_modifier_ambient(modifier);
                    }
                }
                let save_context_flags = self.context_flags;
                self.set_context_flags(ast::NodeFlags::AMBIENT, true);
                let result = self.parse_property_or_method_declaration(pos, modifiers);
                self.context_flags = save_context_flags;
                return result;
            }
            return self.parse_property_or_method_declaration(pos, modifiers);
        }
        if modifiers.is_some() {
            self.parse_error_at(
                self.node_pos(),
                self.node_pos(),
                &diagnostics::DECLARATION_EXPECTED,
                Vec::new(),
            );
            let name = self.create_missing_identifier();
            return self.parse_property_declaration(pos, modifiers, name, None);
        }
        panic!("Should not have attempted to parse class member declaration.")
    }

    pub fn parse_class_static_block_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_expected_token(ast::Kind::StaticKeyword);
        let body = self.parse_class_static_block_body();
        let result = finish_node!(
            self,
            self.factory
                .new_class_static_block_declaration(modifiers, body),
            pos
        );
        result
    }

    pub fn parse_class_static_block_body(&mut self) -> ast::Node {
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::YIELD_CONTEXT, false);
        self.set_context_flags(ast::NodeFlags::AWAIT_CONTEXT, true);
        let body = self.parse_block(false, None);
        self.context_flags = save_context_flags;
        body
    }

    pub fn try_parse_constructor_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> Option<ast::Node> {
        let state = self.mark();
        if self.token == ast::Kind::ConstructorKeyword
            || self.token == ast::Kind::StringLiteral
                && self.scanner.as_ref().expect("scanner").token_value() == "constructor"
                && self.look_ahead(Parser::next_token_is_open_paren)
        {
            self.next_token();
            let type_parameters = self.parse_type_parameters();
            let parameters = self.parse_parameters(crate::ParseFlags::NONE);
            let return_type = self.parse_return_type(ast::Kind::ColonToken, false);
            let body = self.parse_function_block_or_semicolon(
                crate::ParseFlags::NONE,
                Some(&diagnostics::X_OR_EXPECTED),
            );
            let result = finish_node!(
                self,
                self.factory.new_constructor_declaration(
                    modifiers,
                    type_parameters,
                    parameters,
                    return_type,
                    None,
                    body
                ),
                pos
            );
            return Some(self.check_js_syntax(result));
        }
        self.rewind(state);
        None
    }

    pub fn parse_property_or_method_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let asterisk_token = self.parse_optional_token(ast::Kind::AsteriskToken);
        let name = self.parse_property_name();
        let question_token = self.parse_optional_token(ast::Kind::QuestionToken);
        if asterisk_token.is_some()
            || self.token == ast::Kind::OpenParenToken
            || self.token == ast::Kind::LessThanToken
        {
            return self.parse_method_declaration(
                pos,
                modifiers,
                asterisk_token,
                name,
                question_token,
                Some(&diagnostics::X_OR_EXPECTED),
            );
        }
        self.parse_property_declaration(pos, modifiers, name, question_token)
    }

    pub fn parse_method_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
        asterisk_token: Option<ast::Node>,
        name: ast::Node,
        question_token: Option<ast::Node>,
        diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        let signature_flags =
            if asterisk_token.is_some() {
                crate::ParseFlags::YIELD
            } else {
                crate::ParseFlags::NONE
            } | if modifier_list_has_async(self.factory.store(), modifiers.as_ref()) {
                crate::ParseFlags::AWAIT
            } else {
                crate::ParseFlags::NONE
            };
        let type_parameters = self.parse_type_parameters();
        let parameters = self.parse_parameters(signature_flags);
        let type_node = self.parse_return_type(ast::Kind::ColonToken, false);
        let body = self.parse_function_block_or_semicolon(signature_flags, diagnostic_message);
        let result = finish_node!(
            self,
            self.factory.new_method_declaration(
                modifiers,
                asterisk_token,
                name,
                question_token,
                type_parameters,
                parameters,
                type_node,
                None,
                body
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_property_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
        name: ast::Node,
        question_token: Option<ast::Node>,
    ) -> ast::Node {
        let postfix_token = if question_token.is_none() && !self.has_preceding_line_break() {
            self.parse_optional_token(ast::Kind::ExclamationToken)
        } else {
            question_token
        };
        let type_node = self.parse_type_annotation();
        let initializer = do_in_context(
            self,
            ast::NodeFlags::YIELD_CONTEXT
                | ast::NodeFlags::AWAIT_CONTEXT
                | ast::NodeFlags::DISALLOW_IN_CONTEXT,
            false,
            Parser::parse_initializer,
        );
        self.parse_semicolon_after_property_name(&name, type_node.as_ref(), initializer.as_ref());
        let result = finish_node!(
            self,
            self.factory.new_property_declaration(
                modifiers,
                name,
                postfix_token,
                type_node,
                initializer
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_semicolon_after_property_name(
        &mut self,
        name: &ast::Node,
        type_node: Option<&ast::Node>,
        initializer: Option<&ast::Node>,
    ) {
        if self.token == ast::Kind::AtToken && !self.has_preceding_line_break() {
            self.parse_error_at_current_token(&diagnostics::DECORATORS_MUST_PRECEDE_THE_NAME_AND_ALL_KEYWORDS_OF_PROPERTY_DECLARATIONS, Vec::new());
            return;
        }
        if self.token == ast::Kind::OpenParenToken {
            self.parse_error_at_current_token(
                &diagnostics::CANNOT_START_A_FUNCTION_CALL_IN_A_TYPE_ANNOTATION,
                Vec::new(),
            );
            self.next_token();
            return;
        }
        if type_node.is_some() && !self.can_parse_semicolon() {
            if initializer.is_some() {
                self.parse_error_at_current_token(
                    &diagnostics::X_0_EXPECTED,
                    vec![Box::new(scanner::token_to_string(
                        ast::Kind::SemicolonToken,
                    ))],
                );
            } else {
                self.parse_error_at_current_token(
                    &diagnostics::EXPECTED_FOR_PROPERTY_INITIALIZER,
                    Vec::new(),
                );
            }
            return;
        }
        if self.try_parse_semicolon() {
            return;
        }
        if initializer.is_some() {
            self.parse_error_at_current_token(
                &diagnostics::X_0_EXPECTED,
                vec![Box::new(scanner::token_to_string(
                    ast::Kind::SemicolonToken,
                ))],
            );
            return;
        }
        self.parse_error_for_missing_semicolon_after(name);
    }

    pub fn parse_error_for_missing_semicolon_after(&mut self, node: &ast::Node) {
        if self.factory.store().kind(*node) == ast::Kind::TaggedTemplateExpression {
            if let Some(template) = self.factory.store().template(*node) {
                self.parse_error_at_range(
                    self.skip_range_trivia(self.factory.store().loc(template)),
                    &diagnostics::MODULE_DECLARATION_NAMES_MAY_ONLY_USE_OR_QUOTED_STRINGS,
                    Vec::new(),
                );
            }
            return;
        }
        let expression_text = if self.factory.store().kind(*node) == ast::Kind::Identifier {
            self.factory.store().text(*node)
        } else {
            String::new()
        };
        if expression_text.is_empty() {
            self.parse_error_at_current_token(
                &diagnostics::X_0_EXPECTED,
                vec![Box::new(scanner::token_to_string(
                    ast::Kind::SemicolonToken,
                ))],
            );
            return;
        }
        let loc = self.factory.store().loc(*node);
        let pos = scanner::skip_trivia(&self.source_text, loc.pos() as usize) as i32;
        match expression_text.as_str() {
            "const" | "let" | "var" => {
                self.parse_error_at(
                    pos,
                    loc.end(),
                    &diagnostics::VARIABLE_DECLARATION_NOT_ALLOWED_AT_THIS_LOCATION,
                    Vec::new(),
                );
                return;
            }
            "declare" => return,
            "interface" => {
                self.parse_error_for_invalid_name(
                    &diagnostics::INTERFACE_NAME_CANNOT_BE_0,
                    &diagnostics::INTERFACE_MUST_BE_GIVEN_A_NAME,
                    ast::Kind::OpenBraceToken,
                );
                return;
            }
            "is" => {
                self.parse_error_at(
                    pos,
                    self.scanner.as_ref().expect("scanner").token_start(),
                    &diagnostics::A_TYPE_PREDICATE_IS_ONLY_ALLOWED_IN_RETURN_TYPE_POSITION_FOR_FUNCTIONS_AND_METHODS,
                    Vec::new());
                return;
            }
            "module" | "namespace" => {
                self.parse_error_for_invalid_name(
                    &diagnostics::NAMESPACE_NAME_CANNOT_BE_0,
                    &diagnostics::NAMESPACE_MUST_BE_GIVEN_A_NAME,
                    ast::Kind::OpenBraceToken,
                );
                return;
            }
            "type" => {
                self.parse_error_for_invalid_name(
                    &diagnostics::TYPE_ALIAS_NAME_CANNOT_BE_0,
                    &diagnostics::TYPE_ALIAS_MUST_BE_GIVEN_A_NAME,
                    ast::Kind::EqualsToken,
                );
                return;
            }
            _ => {}
        }
        let mut suggestion = core::get_spelling_suggestion_for_strings(
            &expression_text,
            viable_keyword_suggestions().into_iter(),
        );
        if suggestion.is_empty() {
            suggestion = get_space_suggestion(&expression_text);
        }
        if !suggestion.is_empty() {
            self.parse_error_at(
                pos,
                loc.end(),
                &diagnostics::Unknown_keyword_or_identifier_Did_you_mean_0,
                vec![Box::new(suggestion)],
            );
            return;
        }
        if self.token == ast::Kind::Unknown {
            return;
        }
        self.parse_error_at(
            pos,
            loc.end(),
            &diagnostics::UNEXPECTED_KEYWORD_OR_IDENTIFIER,
            Vec::new(),
        );
    }

    pub fn parse_interface_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_expected(ast::Kind::InterfaceKeyword);
        let name = self.parse_identifier();
        let type_parameters = self.parse_type_parameters();
        let heritage_clauses = self.parse_heritage_clauses();
        let members = self.parse_object_type_members();
        let result = finish_node!(
            self,
            self.factory.new_interface_declaration(
                modifiers,
                name,
                type_parameters,
                heritage_clauses,
                members
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_type_alias_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_expected(ast::Kind::TypeKeyword);
        if self.has_preceding_line_break() {
            self.parse_error_at_current_token(
                &diagnostics::LINE_BREAK_NOT_PERMITTED_HERE,
                Vec::new(),
            );
        }
        let name = self.parse_identifier();
        let type_parameters = self.parse_type_parameters();
        self.parse_expected(ast::Kind::EqualsToken);
        let type_node = if self.token == ast::Kind::IntrinsicKeyword
            && self.look_ahead(Parser::next_is_not_dot)
        {
            self.parse_keyword_type_node()
        } else {
            self.parse_type()
        };
        self.parse_semicolon();
        let result = finish_node!(
            self,
            self.factory
                .new_type_alias_declaration(modifiers, name, type_parameters, type_node),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn next_is_not_dot(&mut self) -> bool {
        self.next_token() != ast::Kind::DotToken
    }

    pub fn parse_enum_member(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let name = self.parse_property_name();
        let initializer = do_in_context(
            self,
            ast::NodeFlags::DISALLOW_IN_CONTEXT,
            false,
            Parser::parse_initializer,
        );
        let result = finish_node!(self, self.factory.new_enum_member(name, initializer), pos);
        result
    }

    pub fn parse_enum_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.parse_expected(ast::Kind::EnumKeyword);
        let name = self.parse_identifier();
        let members = if self.parse_expected(ast::Kind::OpenBraceToken) {
            let save_context_flags = self.context_flags;
            self.set_context_flags(
                ast::NodeFlags::YIELD_CONTEXT | ast::NodeFlags::AWAIT_CONTEXT,
                false,
            );
            let members =
                self.parse_delimited_list(ParsingContext::PCEnumMembers, Parser::parse_enum_member);
            self.context_flags = save_context_flags;
            self.parse_expected(ast::Kind::CloseBraceToken);
            members
        } else {
            self.create_missing_list()
        };
        let result = finish_node!(
            self,
            self.factory.new_enum_declaration(modifiers, name, members),
            pos
        );
        let result = self.check_js_syntax(result);
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub fn parse_module_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let mut keyword = ast::Kind::ModuleKeyword;
        if self.token == ast::Kind::GlobalKeyword {
            return self.parse_ambient_external_module_declaration(pos, modifiers);
        } else if self.parse_optional(ast::Kind::NamespaceKeyword) {
            keyword = ast::Kind::NamespaceKeyword;
        } else {
            self.parse_expected(ast::Kind::ModuleKeyword);
            if self.token == ast::Kind::StringLiteral {
                return self.parse_ambient_external_module_declaration(pos, modifiers);
            }
        }
        self.parse_module_or_namespace_declaration(pos, modifiers, false, keyword)
    }

    pub fn parse_ambient_external_module_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let mut keyword = ast::Kind::ModuleKeyword;
        let save_has_await_identifier = self.statement_has_await_identifier;
        let name = if self.token == ast::Kind::GlobalKeyword {
            keyword = ast::Kind::GlobalKeyword;
            self.parse_identifier()
        } else {
            self.parse_literal_expression(true)
        };
        let body = if self.token == ast::Kind::OpenBraceToken {
            Some(self.parse_module_block())
        } else {
            self.parse_semicolon();
            None
        };
        let result = finish_node!(
            self,
            self.factory
                .new_module_declaration(modifiers, keyword, name, body),
            pos
        );
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub fn parse_module_block(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let statements = if self.parse_expected(ast::Kind::OpenBraceToken) {
            let statements =
                self.parse_list(ParsingContext::PCBlockStatements, Parser::parse_statement);
            self.parse_expected(ast::Kind::CloseBraceToken);
            statements
        } else {
            self.create_missing_list()
        };
        finish_node!(self, self.factory.new_module_block(statements), pos)
    }

    pub fn parse_module_or_namespace_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
        nested: bool,
        keyword: ast::Kind,
    ) -> ast::Node {
        let save_has_await_identifier = self.statement_has_await_identifier;
        let name = if nested {
            self.parse_identifier_name()
        } else {
            self.parse_identifier()
        };
        let body = if self.parse_optional(ast::Kind::DotToken) {
            let implicit_export = self.factory.new_modifier(ast::Kind::ExportKeyword);
            let implicit_export_loc = core::TextRange::new(self.node_pos(), self.node_pos());
            self.factory
                .mark_parsed_implicit_export(implicit_export, implicit_export_loc);
            let implicit_modifiers =
                self.new_parser_modifier_list(implicit_export_loc, vec![implicit_export]);
            self.parse_module_or_namespace_declaration(
                self.node_pos(),
                Some(implicit_modifiers),
                true,
                keyword,
            )
        } else {
            self.parse_module_block()
        };
        let result = finish_node!(
            self,
            self.factory
                .new_module_declaration(modifiers, keyword, name, Some(body)),
            pos
        );
        let result = self.check_js_syntax(result);
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub fn parse_parameters(&mut self, flags: crate::ParseFlags) -> ast::NodeList {
        if self.parse_expected(ast::Kind::OpenParenToken) {
            let parameters = self
                .parse_parameters_worker(flags, true)
                .expect("ambiguous parameter parsing always produces a list");
            self.parse_expected(ast::Kind::CloseParenToken);
            return parameters;
        }
        self.create_missing_list()
    }

    pub fn parse_parameters_worker(
        &mut self,
        flags: crate::ParseFlags,
        allow_ambiguity: bool,
    ) -> Option<ast::NodeList> {
        let in_await_context =
            self.context_flags & ast::NodeFlags::AWAIT_CONTEXT != ast::NodeFlags::NONE;
        let save_context_flags = self.context_flags;
        self.set_context_flags(
            ast::NodeFlags::YIELD_CONTEXT,
            flags & crate::ParseFlags::YIELD != crate::ParseFlags::NONE,
        );
        self.set_context_flags(
            ast::NodeFlags::AWAIT_CONTEXT,
            flags & crate::ParseFlags::AWAIT != crate::ParseFlags::NONE,
        );
        let parameters = self.parse_delimited_list_opt(ParsingContext::PCParameters, |parser| {
            let parameter = parser.parse_parameter_ex(in_await_context, allow_ambiguity);
            if !allow_ambiguity && ast::node_is_missing(parser.factory.store(), Some(parameter)) {
                return None;
            }
            if flags & crate::ParseFlags::TYPE == crate::ParseFlags::NONE {
                parser.check_js_syntax(parameter);
            }
            Some(parameter)
        });
        self.context_flags = save_context_flags;
        parameters
    }

    pub fn parse_parameter(&mut self) -> ast::Node {
        self.parse_parameter_ex(false, true)
    }

    pub fn parse_parameter_ex(
        &mut self,
        in_outer_await_context: bool,
        allow_ambiguity: bool,
    ) -> ast::Node {
        let pos = self.node_pos();
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::AWAIT_CONTEXT, in_outer_await_context);
        let modifiers = self.parse_modifiers_ex(true, false, false);
        self.context_flags = save_context_flags;
        if self.token == ast::Kind::ThisKeyword {
            let name = self.create_identifier(true);
            let type_node = self.parse_type_annotation();
            let result = finish_node!(
                self,
                self.factory.new_parameter_declaration(
                    modifiers.clone(),
                    None,
                    name,
                    None,
                    type_node,
                    None
                ),
                pos
            );
            if let Some(modifiers) = modifiers {
                let modifier_loc = self
                    .factory
                    .parsed_modifier_list_nodes(modifiers)
                    .into_iter()
                    .next()
                    .map(|modifier| self.factory.store().loc(modifier))
                    .expect("parameter modifier");
                self.parse_error_at_range(
                    modifier_loc,
                    &diagnostics::NEITHER_DECORATORS_NOR_MODIFIERS_MAY_BE_APPLIED_TO_THIS_PARAMETERS,
                    Vec::new());
            }
            return result;
        }
        let dot_dot_dot_token = self.parse_optional_token(ast::Kind::DotDotDotToken);
        if !allow_ambiguity && !self.is_parameter_name_start() {
            return self.create_missing_identifier();
        }
        let name = self.parse_name_of_parameter(modifiers.as_ref());
        let question_token = self.parse_optional_token(ast::Kind::QuestionToken);
        let type_node = self.parse_type_annotation();
        let initializer = self.parse_initializer();
        let result = finish_node!(
            self,
            self.factory.new_parameter_declaration(
                modifiers,
                dot_dot_dot_token,
                name,
                question_token,
                type_node,
                initializer
            ),
            pos
        );
        result
    }

    pub fn is_parameter_name_start(&mut self) -> bool {
        self.is_binding_identifier()
            || self.token == ast::Kind::OpenBracketToken
            || self.token == ast::Kind::OpenBraceToken
    }

    pub fn parse_name_of_parameter(&mut self, modifiers: Option<&ast::ModifierList>) -> ast::Node {
        let name = self.parse_identifier_or_pattern_with_diagnostic(Some(
            &diagnostics::PRIVATE_IDENTIFIERS_CANNOT_BE_USED_AS_PARAMETERS,
        ));
        if self.factory.store().loc(name).len() == 0
            && modifiers.is_none()
            && ast::is_modifier_kind(self.token)
        {
            self.next_token();
        }
        name
    }

    pub fn parse_return_type(
        &mut self,
        return_token: ast::Kind,
        is_type: bool,
    ) -> Option<ast::Node> {
        if self.should_parse_return_type(return_token, is_type) {
            return Some(do_in_context(
                self,
                ast::NodeFlags::DISALLOW_CONDITIONAL_TYPES_CONTEXT,
                false,
                Parser::parse_type_or_type_predicate,
            ));
        }
        None
    }

    pub fn should_parse_return_type(&mut self, return_token: ast::Kind, is_type: bool) -> bool {
        if return_token == ast::Kind::EqualsGreaterThanToken {
            self.parse_expected(return_token);
            true
        } else if self.parse_optional(ast::Kind::ColonToken) {
            true
        } else if is_type && self.token == ast::Kind::EqualsGreaterThanToken {
            self.parse_error_at_current_token(
                &diagnostics::X_0_EXPECTED,
                vec![Box::new(scanner::token_to_string(ast::Kind::ColonToken))],
            );
            self.next_token();
            true
        } else {
            false
        }
    }

    pub fn parse_type_or_type_predicate(&mut self) -> ast::Node {
        if self.is_identifier() {
            let state = self.mark();
            let pos = self.node_pos();
            let id = self.parse_identifier();
            if self.token == ast::Kind::IsKeyword && !self.has_preceding_line_break() {
                self.next_token();
                let type_node = self.parse_type();
                return finish_node!(
                    self,
                    self.factory
                        .new_type_predicate_node(None, id, Some(type_node)),
                    pos
                );
            }
            self.rewind(state);
        }
        self.parse_type()
    }

    pub fn parse_accessor_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
        kind: ast::Kind,
        flags: crate::ParseFlags,
    ) -> ast::Node {
        let name = self.parse_property_name();
        let type_parameters = self.parse_type_parameters();
        let parameters = self.parse_parameters(crate::ParseFlags::NONE);
        let return_type = self.parse_return_type(ast::Kind::ColonToken, false);
        let body = self.parse_function_block_or_semicolon(flags, None);
        let result = if kind == ast::Kind::GetAccessor {
            self.factory.new_get_accessor_declaration(
                modifiers,
                name,
                type_parameters,
                parameters,
                return_type,
                None,
                body,
            )
        } else {
            self.factory.new_set_accessor_declaration(
                modifiers,
                name,
                type_parameters,
                parameters,
                return_type,
                None,
                body,
            )
        };
        let result = self.finish_node(result, pos);
        if flags & crate::ParseFlags::TYPE == crate::ParseFlags::NONE {
            return self.check_js_syntax(result);
        }
        result
    }

    pub fn parse_function_block_or_semicolon(
        &mut self,
        flags: crate::ParseFlags,
        diagnostic_message: Option<&diagnostics::Message>,
    ) -> Option<ast::Node> {
        if self.token != ast::Kind::OpenBraceToken {
            if flags & crate::ParseFlags::TYPE != crate::ParseFlags::NONE {
                self.parse_type_member_semicolon();
                return None;
            }
            if self.can_parse_semicolon() {
                self.parse_semicolon();
                return None;
            }
        }
        Some(self.parse_function_block_with_diagnostic(flags, diagnostic_message))
    }

    pub fn parse_function_block(&mut self, flags: crate::ParseFlags) -> ast::Node {
        self.parse_function_block_with_diagnostic(flags, None)
    }

    pub fn parse_function_block_with_diagnostic(
        &mut self,
        flags: crate::ParseFlags,
        diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        let save_context_flags = self.context_flags;
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.set_context_flags(
            ast::NodeFlags::YIELD_CONTEXT,
            flags & crate::ParseFlags::YIELD != crate::ParseFlags::NONE,
        );
        self.set_context_flags(
            ast::NodeFlags::AWAIT_CONTEXT,
            flags & crate::ParseFlags::AWAIT != crate::ParseFlags::NONE,
        );
        self.set_context_flags(ast::NodeFlags::DECORATOR_CONTEXT, false);
        let block = self.parse_block(
            flags & crate::ParseFlags::IGNORE_MISSING_OPEN_BRACE != crate::ParseFlags::NONE,
            diagnostic_message,
        );
        self.context_flags = save_context_flags;
        self.statement_has_await_identifier = save_has_await_identifier;
        block
    }

    pub fn is_index_signature(&mut self) -> bool {
        self.token == ast::Kind::OpenBracketToken
            && self.look_ahead(Parser::next_is_unambiguously_index_signature)
    }

    pub fn next_is_unambiguously_index_signature(&mut self) -> bool {
        self.next_token();
        if self.token == ast::Kind::DotDotDotToken || self.token == ast::Kind::CloseBracketToken {
            return true;
        }
        if ast::is_modifier_kind(self.token) {
            self.next_token();
            if self.is_identifier() {
                return true;
            }
        } else if !self.is_identifier() {
            return false;
        } else {
            self.next_token();
        }
        if self.token == ast::Kind::ColonToken || self.token == ast::Kind::CommaToken {
            return true;
        }
        if self.token == ast::Kind::QuestionToken {
            self.next_token();
            return self.token == ast::Kind::ColonToken
                || self.token == ast::Kind::CommaToken
                || self.token == ast::Kind::CloseBracketToken;
        }
        false
    }

    pub fn parse_index_signature_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_expected(ast::Kind::OpenBracketToken);
        let parameters =
            self.parse_delimited_list(ParsingContext::PCParameters, Parser::parse_parameter);
        self.parse_expected(ast::Kind::CloseBracketToken);
        let type_node = self.parse_type_annotation();
        self.parse_type_member_semicolon();
        let result = finish_node!(
            self,
            self.factory
                .new_index_signature_declaration(modifiers, parameters, type_node),
            pos
        );
        result
    }

    pub fn parse_entity_name(
        &mut self,
        allow_reserved_words: bool,
        diagnostic_message: Option<&diagnostics::Message>,
    ) -> ast::Node {
        let pos = self.node_pos();
        let mut entity = if allow_reserved_words {
            self.parse_identifier_name_with_diagnostic(diagnostic_message)
        } else {
            self.parse_identifier_with_diagnostic(diagnostic_message, None)
        };
        while self.parse_optional(ast::Kind::DotToken) {
            if self.token == ast::Kind::LessThanToken {
                break;
            }
            let right = self.parse_right_side_of_dot(allow_reserved_words, false, true);
            entity = finish_node!(self, self.factory.new_qualified_name(entity, right), pos);
        }
        entity
    }

    pub fn parse_right_side_of_dot(
        &mut self,
        allow_identifier_names: bool,
        allow_private_identifiers: bool,
        allow_unicode_escape_sequence_in_identifier_name: bool,
    ) -> ast::Node {
        if self.has_preceding_line_break()
            && token_is_identifier_or_keyword(self.token)
            && self.look_ahead(Parser::next_token_is_identifier_or_keyword_on_same_line)
        {
            self.parse_error_at(
                self.node_pos(),
                self.node_pos(),
                &diagnostics::IDENTIFIER_EXPECTED,
                Vec::new(),
            );
            return self.create_missing_identifier();
        }
        if self.token == ast::Kind::PrivateIdentifier {
            let node = self.parse_private_identifier();
            if allow_private_identifiers {
                return node;
            }
            self.parse_error_at(
                self.node_pos(),
                self.node_pos(),
                &diagnostics::IDENTIFIER_EXPECTED,
                Vec::new(),
            );
            return self.create_missing_identifier();
        }
        if allow_identifier_names {
            if allow_unicode_escape_sequence_in_identifier_name {
                return self.parse_identifier_name();
            }
            return self.parse_identifier_name_error_on_unicode_escape_sequence();
        }
        let save_has_await_identifier = self.statement_has_await_identifier;
        let id = self.parse_identifier();
        self.statement_has_await_identifier = save_has_await_identifier;
        id
    }

    pub fn parse_import_declaration_or_import_equals_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_expected(ast::Kind::ImportKeyword);
        let after_import_pos = self.node_pos();
        let save_has_await_identifier = self.statement_has_await_identifier;
        let mut identifier = if self.is_identifier() {
            Some(self.parse_identifier())
        } else {
            None
        };
        let mut phase_modifier = ast::Kind::Unknown;
        if identifier
            .as_ref()
            .is_some_and(|id| self.factory.store().text_eq(*id, "type"))
            && (self.token != ast::Kind::FromKeyword
                || self.is_identifier()
                    && self.look_ahead(Parser::next_token_is_from_keyword_or_equals_token))
            && (self.is_identifier()
                || self.token_after_import_definitely_produces_import_declaration())
        {
            phase_modifier = ast::Kind::TypeKeyword;
            identifier = if self.is_identifier() {
                Some(self.parse_identifier())
            } else {
                None
            };
        } else if identifier
            .as_ref()
            .is_some_and(|id| self.factory.store().text_eq(*id, "defer"))
        {
            let should_parse_as_defer_modifier = if self.token == ast::Kind::FromKeyword {
                !self.look_ahead(Parser::next_token_is_token_string_literal)
            } else {
                self.token != ast::Kind::CommaToken && self.token != ast::Kind::EqualsToken
            };
            if should_parse_as_defer_modifier {
                phase_modifier = ast::Kind::DeferKeyword;
                identifier = if self.is_identifier() {
                    Some(self.parse_identifier())
                } else {
                    None
                };
            }
        }
        if identifier.is_some()
            && !self.token_after_imported_identifier_definitely_produces_import_declaration()
            && phase_modifier != ast::Kind::DeferKeyword
        {
            let import_equals = self.parse_import_equals_declaration(
                pos,
                modifiers,
                identifier.unwrap(),
                phase_modifier == ast::Kind::TypeKeyword,
            );
            let import_equals = self.check_js_syntax(import_equals);
            self.statement_has_await_identifier = save_has_await_identifier;
            return import_equals;
        }
        let import_clause =
            self.try_parse_import_clause(identifier, after_import_pos, phase_modifier);
        self.statement_has_await_identifier = save_has_await_identifier;
        let module_specifier = self.parse_module_specifier();
        let attributes = self.try_parse_import_attributes();
        self.parse_semicolon();
        let result = finish_node!(
            self,
            self.factory.new_import_declaration(
                modifiers,
                import_clause,
                module_specifier,
                attributes
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn next_token_is_from_keyword_or_equals_token(&mut self) -> bool {
        self.next_token();
        self.token == ast::Kind::FromKeyword || self.token == ast::Kind::EqualsToken
    }

    pub fn token_after_import_definitely_produces_import_declaration(&self) -> bool {
        self.token == ast::Kind::AsteriskToken || self.token == ast::Kind::OpenBraceToken
    }

    pub fn token_after_imported_identifier_definitely_produces_import_declaration(&self) -> bool {
        self.token == ast::Kind::CommaToken || self.token == ast::Kind::FromKeyword
    }

    pub fn parse_import_equals_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
        identifier: ast::Node,
        is_type_only: bool,
    ) -> ast::Node {
        self.parse_expected(ast::Kind::EqualsToken);
        let module_reference = self.parse_module_reference();
        self.parse_semicolon();
        let result = finish_node!(
            self,
            self.factory.new_import_equals_declaration(
                modifiers,
                is_type_only,
                identifier,
                module_reference
            ),
            pos
        );
        result
    }

    pub fn parse_module_reference(&mut self) -> ast::Node {
        if self.token == ast::Kind::RequireKeyword
            && self.look_ahead(Parser::next_token_is_open_paren)
        {
            return self.parse_external_module_reference();
        }
        self.parse_entity_name(false, None)
    }

    pub fn parse_external_module_reference(&mut self) -> ast::Node {
        let save_has_await_identifier = self.statement_has_await_identifier;
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::RequireKeyword);
        self.parse_expected(ast::Kind::OpenParenToken);
        let expression = self.parse_module_specifier();
        self.parse_expected(ast::Kind::CloseParenToken);
        let result = finish_node!(
            self,
            self.factory.new_external_module_reference(expression),
            pos
        );
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub fn parse_module_specifier(&mut self) -> ast::Node {
        if self.token == ast::Kind::StringLiteral {
            return self.parse_literal_expression(true);
        }
        self.parse_expression()
    }

    pub fn try_parse_import_clause(
        &mut self,
        identifier: Option<ast::Node>,
        pos: i32,
        phase_modifier: ast::Kind,
    ) -> Option<ast::Node> {
        if identifier.is_some()
            || self.token == ast::Kind::AsteriskToken
            || self.token == ast::Kind::OpenBraceToken
        {
            let import_clause = self.parse_import_clause(identifier, pos, phase_modifier);
            self.parse_expected(ast::Kind::FromKeyword);
            return Some(import_clause);
        }
        None
    }

    pub fn parse_import_clause(
        &mut self,
        identifier: Option<ast::Node>,
        pos: i32,
        phase_modifier: ast::Kind,
    ) -> ast::Node {
        let save_has_await_identifier = self.statement_has_await_identifier;
        let named_bindings = if identifier.is_none() || self.parse_optional(ast::Kind::CommaToken) {
            let named_bindings = if self.token == ast::Kind::AsteriskToken {
                Some(self.parse_namespace_import())
            } else {
                Some(self.parse_named_imports())
            };
            named_bindings
        } else {
            None
        };
        let result = finish_node!(
            self,
            self.factory
                .new_import_clause(Some(phase_modifier), identifier, named_bindings),
            pos
        );
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub fn parse_namespace_import(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::AsteriskToken);
        self.parse_expected(ast::Kind::AsKeyword);
        let name = self.parse_identifier();
        finish_node!(self, self.factory.new_namespace_import(name), pos)
    }

    pub fn parse_named_imports(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let imports = self.parse_bracketed_list(
            ParsingContext::PCImportOrExportSpecifiers,
            Parser::parse_import_specifier,
            ast::Kind::OpenBraceToken,
            ast::Kind::CloseBraceToken,
        );
        finish_node!(self, self.factory.new_named_imports(imports), pos)
    }

    pub fn parse_import_specifier(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let (is_type_only, property_name, mut name) =
            self.parse_import_or_export_specifier(ast::Kind::ImportSpecifier);
        if self.factory.store().kind(name) != ast::Kind::Identifier {
            let name_loc = self.factory.store().loc(name);
            self.parse_error_at_range(
                self.skip_range_trivia(name_loc),
                &diagnostics::IDENTIFIER_EXPECTED,
                Vec::new(),
            );
            name = finish_node!(self, self.new_identifier(String::new()), name_loc.pos());
        }
        let import_specifier = finish_node!(
            self,
            self.factory
                .new_import_specifier(is_type_only, property_name, name),
            pos
        );
        self.check_js_syntax(import_specifier)
    }

    pub fn parse_import_or_export_specifier(
        &mut self,
        kind: ast::Kind,
    ) -> (bool, Option<ast::Node>, ast::Node) {
        let mut check_identifier_is_keyword = ast::is_keyword(self.token) && !self.is_identifier();
        let mut check_identifier_start = self.scanner.as_ref().expect("scanner").token_start();
        let mut check_identifier_end = self.scanner.as_ref().expect("scanner").token_end();
        let mut is_type_only = false;
        let mut property_name = None;
        let mut can_parse_as_keyword = true;
        let mut name = self.parse_module_export_name(false);
        if self.factory.store().kind(name) == ast::Kind::Identifier
            && self.factory.store().text_eq(name, "type")
        {
            if self.token == ast::Kind::AsKeyword {
                let first_as = self.parse_identifier_name();
                if self.token == ast::Kind::AsKeyword {
                    let second_as = self.parse_identifier_name();
                    if self.can_parse_module_export_name() {
                        is_type_only = true;
                        property_name = Some(first_as);
                        name = self.parse_module_export_name_with_keyword_check(
                            &mut check_identifier_is_keyword,
                            &mut check_identifier_start,
                            &mut check_identifier_end,
                        );
                        can_parse_as_keyword = false;
                    } else {
                        property_name = Some(name);
                        name = second_as;
                        can_parse_as_keyword = false;
                    }
                } else if self.can_parse_module_export_name() {
                    property_name = Some(name);
                    can_parse_as_keyword = false;
                    name = self.parse_module_export_name_with_keyword_check(
                        &mut check_identifier_is_keyword,
                        &mut check_identifier_start,
                        &mut check_identifier_end,
                    );
                } else {
                    is_type_only = true;
                    name = first_as;
                }
            } else if self.can_parse_module_export_name() {
                is_type_only = true;
                name = self.parse_module_export_name_with_keyword_check(
                    &mut check_identifier_is_keyword,
                    &mut check_identifier_start,
                    &mut check_identifier_end,
                );
            }
        }
        if can_parse_as_keyword && self.token == ast::Kind::AsKeyword {
            property_name = Some(name);
            self.parse_expected(ast::Kind::AsKeyword);
            name = self.parse_module_export_name_with_keyword_check(
                &mut check_identifier_is_keyword,
                &mut check_identifier_start,
                &mut check_identifier_end,
            );
        }
        if kind == ast::Kind::ImportSpecifier && check_identifier_is_keyword {
            self.parse_error_at_range(
                core::TextRange::new(check_identifier_start, check_identifier_end),
                &diagnostics::IDENTIFIER_EXPECTED,
                Vec::new(),
            );
        }
        (is_type_only, property_name, name)
    }

    pub fn can_parse_module_export_name(&self) -> bool {
        token_is_identifier_or_keyword(self.token) || self.token == ast::Kind::StringLiteral
    }

    pub fn parse_module_export_name(&mut self, _disallow_keywords: bool) -> ast::Node {
        if self.token == ast::Kind::StringLiteral {
            return self.parse_literal_expression(false);
        }
        self.parse_identifier_name()
    }

    pub fn parse_module_export_name_with_keyword_check(
        &mut self,
        check_identifier_is_keyword: &mut bool,
        check_identifier_start: &mut i32,
        check_identifier_end: &mut i32,
    ) -> ast::Node {
        *check_identifier_is_keyword = ast::is_keyword(self.token) && !self.is_identifier();
        *check_identifier_start = self.scanner.as_ref().expect("scanner").token_start();
        *check_identifier_end = self.scanner.as_ref().expect("scanner").token_end();
        self.parse_module_export_name(false)
    }

    pub fn try_parse_import_attributes(&mut self) -> Option<ast::Node> {
        if self.token == ast::Kind::WithKeyword
            || (self.token == ast::Kind::AssertKeyword && !self.has_preceding_line_break())
        {
            if self.token == ast::Kind::AssertKeyword {
                self.parse_error_at_current_token(
                    &diagnostics::IMPORT_ASSERTIONS_HAVE_BEEN_REPLACED_BY_IMPORT_ATTRIBUTES_USE_WITH_INSTEAD_OF_ASSERT,
                    Vec::new(),
                );
            }
            return Some(self.parse_import_attributes(self.token, false));
        }
        None
    }

    pub fn parse_import_attribute(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let name = if token_is_identifier_or_keyword(self.token) {
            Some(self.parse_identifier_name())
        } else if self.token == ast::Kind::StringLiteral {
            Some(self.parse_literal_expression(false))
        } else {
            None
        };
        if name.is_some() {
            self.parse_expected(ast::Kind::ColonToken);
        } else {
            self.parse_error_at_current_token(
                &diagnostics::IDENTIFIER_OR_STRING_LITERAL_EXPECTED,
                Vec::new(),
            );
        }
        let value = self.parse_assignment_expression_or_higher();
        finish_node!(self, self.factory.new_import_attribute(name, value), pos)
    }

    pub fn parse_import_attributes(&mut self, token: ast::Kind, skip_keyword: bool) -> ast::Node {
        let pos = self.node_pos();
        if !skip_keyword {
            self.parse_expected(token);
        }
        let mut multi_line = false;
        let open_brace_position = self.scanner.as_ref().expect("scanner").token_start();
        let elements = if self.parse_expected(ast::Kind::OpenBraceToken) {
            multi_line = self.has_preceding_line_break();
            let elements = self.parse_delimited_list(
                ParsingContext::PCImportAttributes,
                Parser::parse_import_attribute,
            );
            if !self.parse_expected(ast::Kind::CloseBraceToken) {
                self.add_bracket_related_info(
                    open_brace_position,
                    ast::Kind::OpenBraceToken,
                    ast::Kind::CloseBraceToken,
                );
            }
            elements
        } else {
            self.parse_empty_node_list()
        };
        finish_node!(
            self,
            self.factory
                .new_import_attributes(token, elements, multi_line),
            pos
        )
    }

    pub fn add_bracket_related_info(
        &mut self,
        open_position: i32,
        open_kind: ast::Kind,
        close_kind: ast::Kind,
    ) {
        if let Some(last_diagnostic) = self.diagnostics.last_mut() {
            if last_diagnostic.code() == diagnostics::X_0_EXPECTED.code() {
                let related = ast::new_diagnostic(
                    None,
                    core::TextRange::new(open_position, open_position),
                    &diagnostics::THE_PARSER_EXPECTED_TO_FIND_A_1_TO_MATCH_THE_0_TOKEN_HERE,
                    &[
                        diagnostics::Any::from(scanner::token_to_string(open_kind)),
                        diagnostics::Any::from(scanner::token_to_string(close_kind)),
                    ],
                );
                last_diagnostic.add_related_info(Some(related));
            }
        }
    }

    pub fn parse_export_assignment(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let save_context_flags = self.context_flags;
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.set_context_flags(ast::NodeFlags::AWAIT_CONTEXT, true);
        let is_export_equals = if self.parse_optional(ast::Kind::EqualsToken) {
            true
        } else {
            self.parse_expected(ast::Kind::DefaultKeyword);
            false
        };
        let expression = self.parse_assignment_expression_or_higher();
        self.parse_semicolon();
        self.context_flags = save_context_flags;
        self.statement_has_await_identifier = save_has_await_identifier;
        let result = finish_node!(
            self,
            self.factory
                .new_export_assignment(modifiers, is_export_equals, None, expression),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_namespace_export_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        self.parse_expected(ast::Kind::AsKeyword);
        self.parse_expected(ast::Kind::NamespaceKeyword);
        let save_has_await_identifier = self.statement_has_await_identifier;
        let name = self.parse_identifier();
        self.statement_has_await_identifier = save_has_await_identifier;
        self.parse_semicolon();
        let result = finish_node!(
            self,
            self.factory
                .new_namespace_export_declaration(modifiers, name),
            pos
        );
        result
    }

    pub fn parse_export_declaration(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let save_context_flags = self.context_flags;
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.set_context_flags(ast::NodeFlags::AWAIT_CONTEXT, true);
        let is_type_only = self.parse_optional(ast::Kind::TypeKeyword);
        let namespace_export_pos = self.node_pos();
        let (export_clause, module_specifier) = if self.parse_optional(ast::Kind::AsteriskToken) {
            let export_clause = if self.parse_optional(ast::Kind::AsKeyword) {
                Some(self.parse_namespace_export(namespace_export_pos))
            } else {
                None
            };
            self.parse_expected(ast::Kind::FromKeyword);
            (export_clause, Some(self.parse_module_specifier()))
        } else {
            let export_clause = Some(self.parse_named_exports());
            let module_specifier = if self.token == ast::Kind::FromKeyword
                || (self.token == ast::Kind::StringLiteral && !self.has_preceding_line_break())
            {
                self.parse_expected(ast::Kind::FromKeyword);
                Some(self.parse_module_specifier())
            } else {
                None
            };
            (export_clause, module_specifier)
        };
        let attributes = if module_specifier.is_some()
            && (self.token == ast::Kind::WithKeyword || self.token == ast::Kind::AssertKeyword)
            && !self.has_preceding_line_break()
        {
            if self.token == ast::Kind::AssertKeyword {
                self.parse_error_at_current_token(
                    &diagnostics::IMPORT_ASSERTIONS_HAVE_BEEN_REPLACED_BY_IMPORT_ATTRIBUTES_USE_WITH_INSTEAD_OF_ASSERT,
                    Vec::new(),
                );
            }
            Some(self.parse_import_attributes(self.token, false))
        } else {
            None
        };
        self.parse_semicolon();
        self.context_flags = save_context_flags;
        self.statement_has_await_identifier = save_has_await_identifier;
        let result = finish_node!(
            self,
            self.factory.new_export_declaration(
                modifiers,
                is_type_only,
                export_clause,
                module_specifier,
                attributes
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_namespace_export(&mut self, pos: i32) -> ast::Node {
        let export_name = self.parse_module_export_name(false);
        finish_node!(self, self.factory.new_namespace_export(export_name), pos)
    }

    pub fn parse_named_exports(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let exports = self.parse_bracketed_list(
            ParsingContext::PCImportOrExportSpecifiers,
            Parser::parse_export_specifier,
            ast::Kind::OpenBraceToken,
            ast::Kind::CloseBraceToken,
        );
        finish_node!(self, self.factory.new_named_exports(exports), pos)
    }

    pub fn parse_export_specifier(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let (is_type_only, property_name, name) =
            self.parse_import_or_export_specifier(ast::Kind::ExportSpecifier);
        let result = finish_node!(
            self,
            self.factory
                .new_export_specifier(is_type_only, property_name, name),
            pos
        );
        self.check_js_syntax(result)
    }
}

fn parsing_context_from_i32(kind: i32) -> ParsingContext {
    match kind {
        0 => ParsingContext::PCSourceElements,
        1 => ParsingContext::PCBlockStatements,
        2 => ParsingContext::PCSwitchClauses,
        3 => ParsingContext::PCSwitchClauseStatements,
        4 => ParsingContext::PCTypeMembers,
        5 => ParsingContext::PCClassMembers,
        6 => ParsingContext::PCEnumMembers,
        7 => ParsingContext::PCHeritageClauseElement,
        8 => ParsingContext::PCVariableDeclarations,
        9 => ParsingContext::PCObjectBindingElements,
        10 => ParsingContext::PCArrayBindingElements,
        11 => ParsingContext::PCArgumentExpressions,
        12 => ParsingContext::PCObjectLiteralMembers,
        13 => ParsingContext::PCJsxAttributes,
        14 => ParsingContext::PCJsxChildren,
        15 => ParsingContext::PCArrayLiteralMembers,
        16 => ParsingContext::PCParameters,
        17 => ParsingContext::PCRestProperties,
        18 => ParsingContext::PCTypeParameters,
        19 => ParsingContext::PCTypeArguments,
        20 => ParsingContext::PCTupleElementTypes,
        21 => ParsingContext::PCHeritageClauses,
        22 => ParsingContext::PCImportOrExportSpecifiers,
        23 => ParsingContext::PCImportAttributes,
        _ => panic!("Unhandled parsing context"),
    }
}

pub fn token_is_identifier_or_keyword(token: ast::Kind) -> bool {
    token >= ast::Kind::Identifier
}

pub fn token_is_identifier_or_keyword_or_greater_than(token: ast::Kind) -> bool {
    token_is_identifier_or_keyword(token) || token == ast::Kind::GreaterThanToken
}

pub fn is_keyword_or_punctuation(kind: ast::Kind) -> bool {
    ast::is_keyword_kind(kind) || ast::is_punctuation_kind(kind)
}

pub fn is_reserved_word(token: ast::Kind) -> bool {
    ast::Kind::FirstReservedWord <= token && token <= ast::Kind::LastReservedWord
}

pub fn is_declare_modifier(store: &ast::AstStore, modifier: &ast::Node) -> bool {
    store.kind(*modifier) == ast::Kind::DeclareKeyword
}

pub fn is_export_modifier(store: &ast::AstStore, modifier: &ast::Node) -> bool {
    store.kind(*modifier) == ast::Kind::ExportKeyword
}

pub fn is_async_modifier(store: &ast::AstStore, modifier: &ast::Node) -> bool {
    store.kind(*modifier) == ast::Kind::AsyncKeyword
}

pub fn modifier_list_has_async(
    store: &ast::AstStore,
    modifiers: Option<&ast::ModifierList>,
) -> bool {
    modifiers.is_some_and(|modifiers| {
        store
            .parser_access()
            .modifier_list_nodes(*modifiers)
            .into_iter()
            .any(|modifier| is_async_modifier(store, &modifier))
    })
}

pub fn do_in_context<T>(
    p: &mut Parser,
    flags: ast::NodeFlags,
    value: bool,
    f: fn(&mut Parser) -> T,
) -> T {
    let save_context_flags = p.context_flags;
    p.set_context_flags(flags, value);
    let result = f(p);
    p.context_flags = save_context_flags;
    result
}

impl Parser {
    pub fn parse_modifiers(&mut self) -> Option<ast::ModifierList> {
        self.parse_modifiers_ex(false, false, false)
    }

    pub fn parse_modifiers_ex(
        &mut self,
        allow_decorators: bool,
        permit_const_as_modifier: bool,
        stop_on_start_of_class_static_block: bool,
    ) -> Option<ast::ModifierList> {
        let mut has_leading_modifier = false;
        let mut has_trailing_decorator = false;
        let mut has_trailing_modifier = false;
        let mut has_static_modifier = false;
        let pos = self.node_pos();
        let mut list = SmallVec::<[ast::Node; 16]>::new();
        loop {
            if allow_decorators && self.token == ast::Kind::AtToken && !has_trailing_modifier {
                let decorator = self.parse_decorator();
                list.push(decorator);
                if has_leading_modifier {
                    has_trailing_decorator = true;
                }
            } else if let Some(modifier) = self.try_parse_modifier(
                has_static_modifier,
                permit_const_as_modifier,
                stop_on_start_of_class_static_block,
            ) {
                if self.factory.store().kind(modifier) == ast::Kind::StaticKeyword {
                    has_static_modifier = true;
                }
                list.push(modifier);
                if has_trailing_decorator {
                    has_trailing_modifier = true;
                } else {
                    has_leading_modifier = true;
                }
            } else {
                break;
            }
        }
        if list.is_empty() {
            None
        } else {
            Some(self.new_parser_modifier_list(
                core::TextRange::new(pos, self.node_pos()),
                list.into_vec(),
            ))
        }
    }

    pub fn parse_decorator(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::AtToken);
        let expression = do_in_context(
            self,
            ast::NodeFlags::DECORATOR_CONTEXT,
            true,
            Parser::parse_decorator_expression,
        );
        finish_node!(self, self.factory.new_decorator(expression), pos)
    }

    pub fn parse_decorator_expression(&mut self) -> ast::Node {
        if self.in_await_context() && self.token == ast::Kind::AwaitKeyword {
            let pos = self.node_pos();
            let await_expression = self
                .parse_identifier_with_diagnostic(Some(&diagnostics::EXPRESSION_EXPECTED), None);
            self.next_token();
            let member_expression = self.parse_member_expression_rest(pos, await_expression, true);
            return self.parse_call_expression_rest(pos, member_expression);
        }
        self.parse_left_hand_side_expression_or_higher()
    }

    pub fn try_parse_modifier(
        &mut self,
        has_seen_static_modifier: bool,
        permit_const_as_modifier: bool,
        stop_on_start_of_class_static_block: bool,
    ) -> Option<ast::Node> {
        let pos = self.node_pos();
        let kind = self.token;
        if self.token == ast::Kind::ConstKeyword && permit_const_as_modifier {
            if !self.look_ahead(Parser::next_token_is_on_same_line_and_can_follow_modifier) {
                return None;
            }
            self.next_token();
        } else if stop_on_start_of_class_static_block
            && self.token == ast::Kind::StaticKeyword
            && self.look_ahead(Parser::next_token_is_open_brace)
        {
            return None;
        } else if has_seen_static_modifier && self.token == ast::Kind::StaticKeyword {
            return None;
        } else if !self.parse_any_contextual_modifier() {
            return None;
        }
        Some(finish_node!(self, self.factory.new_modifier(kind), pos))
    }

    pub fn parse_contextual_modifier(&mut self, kind: ast::Kind) -> bool {
        let state = self.mark();
        if self.token == kind && self.next_token_can_follow_modifier() {
            return true;
        }
        self.rewind(state);
        false
    }

    pub fn parse_any_contextual_modifier(&mut self) -> bool {
        let state = self.mark();
        if ast::is_modifier_kind(self.token) && self.next_token_can_follow_modifier() {
            return true;
        }
        self.rewind(state);
        false
    }

    pub fn next_token_can_follow_modifier(&mut self) -> bool {
        match self.token {
            ast::Kind::ConstKeyword => self.next_token() == ast::Kind::EnumKeyword,
            ast::Kind::ExportKeyword => {
                self.next_token();
                if self.token == ast::Kind::DefaultKeyword {
                    return self.look_ahead(Parser::next_token_can_follow_default_keyword);
                }
                if self.token == ast::Kind::TypeKeyword {
                    return self.look_ahead(Parser::next_token_can_follow_export_modifier);
                }
                self.can_follow_export_modifier()
            }
            ast::Kind::DefaultKeyword => self.next_token_can_follow_default_keyword(),
            ast::Kind::StaticKeyword => {
                self.next_token();
                self.can_follow_modifier()
            }
            ast::Kind::GetKeyword | ast::Kind::SetKeyword => {
                self.next_token();
                self.can_follow_get_or_set_keyword()
            }
            _ => self.next_token_is_on_same_line_and_can_follow_modifier(),
        }
    }

    pub fn next_token_can_follow_default_keyword(&mut self) -> bool {
        match self.next_token() {
            ast::Kind::ClassKeyword
            | ast::Kind::FunctionKeyword
            | ast::Kind::InterfaceKeyword
            | ast::Kind::AtToken => true,
            ast::Kind::AbstractKeyword => {
                self.look_ahead(Parser::next_token_is_class_keyword_on_same_line)
            }
            ast::Kind::AsyncKeyword => {
                self.look_ahead(Parser::next_token_is_function_keyword_on_same_line)
            }
            _ => false,
        }
    }

    pub fn next_token_is_class_keyword_on_same_line(&mut self) -> bool {
        self.next_token() == ast::Kind::ClassKeyword && !self.has_preceding_line_break()
    }

    pub fn next_token_can_follow_export_modifier(&mut self) -> bool {
        self.next_token();
        self.can_follow_export_modifier()
    }

    pub fn next_token_is_on_same_line_and_can_follow_modifier(&mut self) -> bool {
        self.next_token();
        !self.has_preceding_line_break() && self.can_follow_modifier()
    }

    pub fn can_follow_export_modifier(&mut self) -> bool {
        self.token == ast::Kind::AtToken
            || self.token != ast::Kind::AsteriskToken
                && self.token != ast::Kind::AsKeyword
                && self.token != ast::Kind::OpenBraceToken
                && self.can_follow_modifier()
    }

    pub fn can_follow_modifier(&mut self) -> bool {
        self.token == ast::Kind::OpenBracketToken
            || self.token == ast::Kind::OpenBraceToken
            || self.token == ast::Kind::AsteriskToken
            || self.token == ast::Kind::DotDotDotToken
            || self.is_literal_property_name()
    }

    pub fn can_follow_get_or_set_keyword(&mut self) -> bool {
        self.token == ast::Kind::OpenBracketToken || self.is_literal_property_name()
    }

    pub fn parse_type(&mut self) -> ast::Node {
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::TYPE_EXCLUDES_FLAGS, false);
        let type_node = if self.is_start_of_function_type_or_constructor_type() {
            self.parse_function_or_constructor_type()
        } else {
            let pos = self.node_pos();
            let mut node = self.parse_union_type_or_higher();
            if !self.in_disallow_conditional_types_context()
                && !self.has_preceding_line_break()
                && self.parse_optional(ast::Kind::ExtendsKeyword)
            {
                let extends_type = do_in_context(
                    self,
                    ast::NodeFlags::DISALLOW_CONDITIONAL_TYPES_CONTEXT,
                    true,
                    Parser::parse_type,
                );
                self.parse_expected(ast::Kind::QuestionToken);
                let true_type = do_in_context(
                    self,
                    ast::NodeFlags::DISALLOW_CONDITIONAL_TYPES_CONTEXT,
                    false,
                    Parser::parse_type,
                );
                self.parse_expected(ast::Kind::ColonToken);
                let false_type = do_in_context(
                    self,
                    ast::NodeFlags::DISALLOW_CONDITIONAL_TYPES_CONTEXT,
                    false,
                    Parser::parse_type,
                );
                node = finish_node!(
                    self,
                    self.factory.new_conditional_type_node(
                        node,
                        extends_type,
                        true_type,
                        false_type
                    ),
                    pos
                );
            }
            node
        };
        self.context_flags = save_context_flags;
        type_node
    }

    pub fn parse_union_type_or_higher(&mut self) -> ast::Node {
        self.parse_union_or_intersection_type(
            ast::Kind::BarToken,
            Parser::parse_intersection_type_or_higher,
        )
    }

    pub fn parse_intersection_type_or_higher(&mut self) -> ast::Node {
        self.parse_union_or_intersection_type(
            ast::Kind::AmpersandToken,
            Parser::parse_type_operator_or_higher,
        )
    }

    pub fn parse_union_or_intersection_type(
        &mut self,
        operator: ast::Kind,
        parse_constituent_type: fn(&mut Parser) -> ast::Node,
    ) -> ast::Node {
        let pos = self.node_pos();
        let is_union_type = operator == ast::Kind::BarToken;
        let has_leading_operator = self.parse_optional(operator);
        let mut type_node = if has_leading_operator {
            self.parse_function_or_constructor_type_to_error(is_union_type, parse_constituent_type)
        } else {
            parse_constituent_type(self)
        };
        if self.token == operator || has_leading_operator {
            let mut types = vec![type_node];
            while self.parse_optional(operator) {
                types.push(self.parse_function_or_constructor_type_to_error(
                    is_union_type,
                    parse_constituent_type,
                ));
            }
            let list = self.new_parser_node_list(core::TextRange::new(pos, self.node_pos()), types);
            type_node = finish_node!(
                self,
                self.create_union_or_intersection_type_node(operator, list),
                pos
            );
        }
        type_node
    }

    pub fn create_union_or_intersection_type_node(
        &mut self,
        operator: ast::Kind,
        types: ast::NodeList,
    ) -> ast::Node {
        match operator {
            ast::Kind::BarToken => self.factory.new_union_type_node(types),
            ast::Kind::AmpersandToken => self.factory.new_intersection_type_node(types),
            _ => panic!("Unhandled case in createUnionOrIntersectionType"),
        }
    }

    pub fn parse_type_operator_or_higher(&mut self) -> ast::Node {
        let operator = self.token;
        match operator {
            ast::Kind::KeyOfKeyword | ast::Kind::UniqueKeyword | ast::Kind::ReadonlyKeyword => {
                self.parse_type_operator(operator)
            }
            ast::Kind::InferKeyword => self.parse_infer_type(),
            _ => do_in_context(
                self,
                ast::NodeFlags::DISALLOW_CONDITIONAL_TYPES_CONTEXT,
                false,
                Parser::parse_postfix_type_or_higher,
            ),
        }
    }

    pub fn parse_type_operator(&mut self, operator: ast::Kind) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(operator);
        let type_node = self.parse_type_operator_or_higher();
        finish_node!(
            self,
            self.factory.new_type_operator_node(operator, type_node),
            pos
        )
    }

    pub fn parse_infer_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::InferKeyword);
        let type_parameter = self.parse_type_parameter_of_infer_type();
        finish_node!(self, self.factory.new_infer_type_node(type_parameter), pos)
    }

    pub fn parse_type_parameter_of_infer_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let name = self.parse_identifier();
        let constraint = self.try_parse_constraint_of_infer_type();
        finish_node!(
            self,
            self.factory.new_type_parameter_declaration(
                None::<ast::ModifierList>,
                name,
                constraint,
                None::<ast::Node>,
                None::<ast::Node>
            ),
            pos
        )
    }

    pub fn try_parse_constraint_of_infer_type(&mut self) -> Option<ast::Node> {
        let state = self.mark();
        if self.parse_optional(ast::Kind::ExtendsKeyword) {
            let constraint = do_in_context(
                self,
                ast::NodeFlags::DISALLOW_CONDITIONAL_TYPES_CONTEXT,
                true,
                Parser::parse_type,
            );
            if self.in_disallow_conditional_types_context()
                || self.token != ast::Kind::QuestionToken
            {
                return Some(constraint);
            }
        }
        self.rewind(state);
        None
    }

    pub fn parse_postfix_type_or_higher(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let mut type_node = self.parse_non_array_type();
        while !self.has_preceding_line_break() {
            match self.token {
                ast::Kind::ExclamationToken => {
                    let loc = self.factory.store().loc(type_node);
                    let start = self.skip_range_trivia(loc).pos();
                    self.next_token();
                    let wrapper_loc = core::TextRange::new(start, self.node_pos());
                    self.factory.finish_parsed_node_header(
                        type_node,
                        wrapper_loc,
                        self.context_flags,
                        false,
                    );
                }
                ast::Kind::QuestionToken => {
                    if self.look_ahead(Parser::next_is_start_of_type) {
                        return type_node;
                    }
                    self.next_token();
                    let loc = self.factory.store().loc(type_node);
                    let start = self.skip_range_trivia(loc).pos();
                    let wrapper_loc = core::TextRange::new(start, self.node_pos());
                    self.factory.finish_parsed_node_header(
                        type_node,
                        wrapper_loc,
                        self.context_flags,
                        false,
                    );
                }
                ast::Kind::OpenBracketToken => {
                    self.parse_expected(ast::Kind::OpenBracketToken);
                    if self.is_start_of_type(false) {
                        let index_type = self.parse_type();
                        self.parse_expected(ast::Kind::CloseBracketToken);
                        type_node = finish_node!(
                            self,
                            self.factory
                                .new_indexed_access_type_node(type_node, index_type),
                            pos
                        );
                    } else {
                        self.parse_expected(ast::Kind::CloseBracketToken);
                        type_node =
                            finish_node!(self, self.factory.new_array_type_node(type_node), pos);
                    }
                }
                _ => return type_node,
            }
        }
        type_node
    }

    pub fn next_is_start_of_type(&mut self) -> bool {
        self.next_token();
        self.is_start_of_type(false)
    }

    pub fn parse_non_array_type(&mut self) -> ast::Node {
        match self.token {
            ast::Kind::AnyKeyword
            | ast::Kind::UnknownKeyword
            | ast::Kind::StringKeyword
            | ast::Kind::NumberKeyword
            | ast::Kind::BigIntKeyword
            | ast::Kind::SymbolKeyword
            | ast::Kind::BooleanKeyword
            | ast::Kind::UndefinedKeyword
            | ast::Kind::NeverKeyword
            | ast::Kind::ObjectKeyword => {
                let state = self.mark();
                let keyword_type_node = self.parse_keyword_type_node();
                if self.token != ast::Kind::DotToken {
                    return keyword_type_node;
                }
                self.rewind(state);
                self.parse_type_reference()
            }
            ast::Kind::VoidKeyword => self.parse_keyword_type_node(),
            ast::Kind::ThisKeyword => {
                let this_keyword = self.parse_this_type_node();
                if self.token == ast::Kind::IsKeyword && !self.has_preceding_line_break() {
                    return self.parse_this_type_predicate(this_keyword);
                }
                this_keyword
            }
            ast::Kind::OpenBraceToken => {
                if self.look_ahead(Parser::next_is_start_of_mapped_type) {
                    return self.parse_mapped_type();
                }
                self.parse_type_literal()
            }
            ast::Kind::OpenBracketToken => self.parse_tuple_type(),
            ast::Kind::OpenParenToken => self.parse_parenthesized_type(),
            ast::Kind::TypeOfKeyword => {
                if self.look_ahead(Parser::next_is_start_of_type_of_import_type) {
                    return self.parse_import_type();
                }
                self.parse_type_query()
            }
            ast::Kind::ImportKeyword => self.parse_import_type(),
            ast::Kind::AssertsKeyword => {
                if self.look_ahead(Parser::next_token_is_identifier_or_keyword_on_same_line) {
                    return self.parse_asserts_type_predicate();
                }
                self.parse_type_reference()
            }
            ast::Kind::TemplateHead => self.parse_template_type(),
            ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::StringLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::TrueKeyword
            | ast::Kind::FalseKeyword
            | ast::Kind::NullKeyword => self.parse_literal_type_node(false),
            ast::Kind::MinusToken => {
                if self.look_ahead(Parser::next_token_is_numeric_or_big_int_literal) {
                    self.parse_literal_type_node(true)
                } else {
                    self.parse_type_reference()
                }
            }
            ast::Kind::QuestionQuestionToken => {
                self.scanner
                    .as_mut()
                    .expect("scanner")
                    .rescan_question_token();
                self.parse_jsdoc_nullable_type()
            }
            ast::Kind::QuestionToken => self.parse_jsdoc_nullable_type(),
            ast::Kind::ExclamationToken => {
                let pos = self.scanner.as_ref().expect("scanner").token_start();
                self.next_token();
                let type_node = self.parse_type_operator_or_higher();
                let wrapper_loc = core::TextRange::new(pos, self.node_pos());
                self.factory.finish_parsed_node_header(
                    type_node,
                    wrapper_loc,
                    self.context_flags,
                    false,
                );
                type_node
            }
            _ => self.parse_type_reference(),
        }
    }

    pub fn parse_jsdoc_nullable_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.next_token();
        let type_node = self.parse_type_operator_or_higher();
        let wrapper_loc = core::TextRange::new(pos, self.node_pos());
        self.factory
            .finish_parsed_node_header(type_node, wrapper_loc, self.context_flags, false);
        type_node
    }

    pub fn parse_keyword_type_node(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let kind = self.token;
        self.next_token();
        finish_node!(self, self.factory.new_keyword_type_node(kind), pos)
    }

    pub fn parse_this_type_node(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.next_token();
        finish_node!(self, self.factory.new_this_type_node(), pos)
    }

    pub fn parse_this_type_predicate(&mut self, lhs: ast::Node) -> ast::Node {
        self.next_token();
        // PORT NOTE: reshaped for borrowck
        let type_node = self.parse_type();
        finish_node!(
            self,
            self.factory
                .new_type_predicate_node(None, lhs.clone(), Some(type_node)),
            self.factory.store().loc(lhs).pos()
        )
    }

    pub fn parse_type_reference(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let type_name = self.parse_entity_name_of_type_reference();
        let type_arguments = self.parse_type_arguments_of_type_reference();
        finish_node!(
            self,
            self.factory
                .new_type_reference_node(type_name, type_arguments),
            pos
        )
    }

    pub fn parse_entity_name_of_type_reference(&mut self) -> ast::Node {
        self.parse_entity_name(true, Some(&diagnostics::TYPE_EXPECTED))
    }

    pub fn parse_type_arguments_of_type_reference(&mut self) -> Option<ast::NodeList> {
        if !self.has_preceding_line_break()
            && self.rescan_less_than_token() == ast::Kind::LessThanToken
        {
            return self.parse_type_arguments();
        }
        None
    }

    pub fn parse_type_arguments(&mut self) -> Option<ast::NodeList> {
        if self.token == ast::Kind::LessThanToken {
            return Some(self.parse_bracketed_list(
                ParsingContext::PCTypeArguments,
                Parser::parse_type,
                ast::Kind::LessThanToken,
                ast::Kind::GreaterThanToken,
            ));
        }
        None
    }

    pub fn next_is_start_of_type_of_import_type(&mut self) -> bool {
        self.next_token();
        self.token == ast::Kind::ImportKeyword
    }

    pub fn parse_type_query(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::TypeOfKeyword);
        let expr_name = self.parse_entity_name(true, None);
        let type_arguments = if !self.has_preceding_line_break() {
            self.parse_type_arguments()
        } else {
            None
        };
        finish_node!(
            self,
            self.factory.new_type_query_node(expr_name, type_arguments),
            pos
        )
    }

    pub fn next_is_start_of_mapped_type(&mut self) -> bool {
        self.next_token();
        if self.token == ast::Kind::PlusToken || self.token == ast::Kind::MinusToken {
            return self.next_token() == ast::Kind::ReadonlyKeyword;
        }
        if self.token == ast::Kind::ReadonlyKeyword {
            self.next_token();
        }
        self.token == ast::Kind::OpenBracketToken
            && self.next_token_is_identifier()
            && self.next_token() == ast::Kind::InKeyword
    }

    pub fn parse_mapped_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenBraceToken);
        let readonly_token = if matches!(
            self.token,
            ast::Kind::ReadonlyKeyword | ast::Kind::PlusToken | ast::Kind::MinusToken
        ) {
            let token = self.parse_token_node();
            if self.factory.store().kind(token) != ast::Kind::ReadonlyKeyword {
                self.parse_expected(ast::Kind::ReadonlyKeyword);
            }
            Some(token)
        } else {
            None
        };
        self.parse_expected(ast::Kind::OpenBracketToken);
        let type_parameter = self.parse_mapped_type_parameter();
        let name_type = if self.parse_optional(ast::Kind::AsKeyword) {
            Some(self.parse_type())
        } else {
            None
        };
        self.parse_expected(ast::Kind::CloseBracketToken);
        let question_token = if matches!(
            self.token,
            ast::Kind::QuestionToken | ast::Kind::PlusToken | ast::Kind::MinusToken
        ) {
            let token = self.parse_token_node();
            if self.factory.store().kind(token) != ast::Kind::QuestionToken {
                self.parse_expected(ast::Kind::QuestionToken);
            }
            Some(token)
        } else {
            None
        };
        let type_node = self.parse_type_annotation();
        self.parse_semicolon();
        let members = self.parse_list(ParsingContext::PCTypeMembers, Parser::parse_type_member);
        self.parse_expected(ast::Kind::CloseBraceToken);
        finish_node!(
            self,
            self.factory.new_mapped_type_node(
                readonly_token,
                type_parameter,
                name_type,
                question_token,
                type_node,
                Some(members)
            ),
            pos
        )
    }

    pub fn parse_mapped_type_parameter(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let name = self.parse_identifier_name();
        self.parse_expected(ast::Kind::InKeyword);
        let type_node = self.parse_type();
        finish_node!(
            self,
            self.factory.new_type_parameter_declaration(
                None::<ast::ModifierList>,
                name,
                Some(type_node),
                None::<ast::Node>,
                None::<ast::Node>
            ),
            pos
        )
    }

    pub fn parse_type_literal(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let members = self.parse_object_type_members();
        finish_node!(self, self.factory.new_type_literal_node(members), pos)
    }

    pub fn parse_object_type_members(&mut self) -> ast::NodeList {
        if self.parse_expected(ast::Kind::OpenBraceToken) {
            let members = self.parse_list(ParsingContext::PCTypeMembers, Parser::parse_type_member);
            self.parse_expected(ast::Kind::CloseBraceToken);
            return members;
        }
        self.create_missing_list()
    }

    pub fn parse_type_member(&mut self) -> ast::Node {
        if self.token == ast::Kind::OpenParenToken || self.token == ast::Kind::LessThanToken {
            return self.parse_signature_member(ast::Kind::CallSignature);
        }
        if self.token == ast::Kind::NewKeyword
            && self.look_ahead(Parser::next_token_is_open_paren_or_less_than)
        {
            return self.parse_signature_member(ast::Kind::ConstructSignature);
        }
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers();
        if self.parse_contextual_modifier(ast::Kind::GetKeyword) {
            return self.parse_accessor_declaration(
                pos,
                modifiers,
                ast::Kind::GetAccessor,
                crate::ParseFlags::TYPE,
            );
        }
        if self.parse_contextual_modifier(ast::Kind::SetKeyword) {
            return self.parse_accessor_declaration(
                pos,
                modifiers,
                ast::Kind::SetAccessor,
                crate::ParseFlags::TYPE,
            );
        }
        if self.is_index_signature() {
            return self.parse_index_signature_declaration(pos, modifiers);
        }
        self.parse_property_or_method_signature(pos, modifiers)
    }

    pub fn parse_property_or_method_signature(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
    ) -> ast::Node {
        let name = self.parse_property_name();
        let question_token = self.parse_optional_token(ast::Kind::QuestionToken);
        let result =
            if self.token == ast::Kind::OpenParenToken || self.token == ast::Kind::LessThanToken {
                let type_parameters = self.parse_type_parameters();
                let parameters = self.parse_parameters(crate::ParseFlags::TYPE);
                let return_type = self.parse_return_type(ast::Kind::ColonToken, true);
                self.factory.new_method_signature_declaration(
                    modifiers,
                    name,
                    question_token,
                    type_parameters,
                    parameters,
                    return_type,
                )
            } else {
                let type_node = self.parse_type_annotation();
                let initializer = if self.token == ast::Kind::EqualsToken {
                    self.parse_initializer()
                } else {
                    None
                };
                self.factory.new_property_signature_declaration(
                    modifiers,
                    name,
                    question_token,
                    type_node,
                    initializer,
                )
            };
        self.parse_type_member_semicolon();
        let result = self.finish_node(result, pos);
        result
    }

    pub fn parse_signature_member(&mut self, kind: ast::Kind) -> ast::Node {
        let pos = self.node_pos();
        if kind == ast::Kind::ConstructSignature {
            self.parse_expected(ast::Kind::NewKeyword);
        }
        let type_parameters = self.parse_type_parameters();
        let parameters = self.parse_parameters(crate::ParseFlags::TYPE);
        let type_node = self.parse_return_type(ast::Kind::ColonToken, true);
        self.parse_type_member_semicolon();
        let result = if kind == ast::Kind::CallSignature {
            self.factory
                .new_call_signature_declaration(type_parameters, parameters, type_node)
        } else {
            self.factory
                .new_construct_signature_declaration(type_parameters, parameters, type_node)
        };
        let result = self.finish_node(result, pos);
        result
    }

    pub fn parse_type_member_semicolon(&mut self) {
        if self.parse_optional(ast::Kind::CommaToken)
            || self.parse_optional(ast::Kind::SemicolonToken)
        {
            return;
        }
        self.parse_semicolon();
    }

    pub fn parse_tuple_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let elements = self.parse_bracketed_list(
            ParsingContext::PCTupleElementTypes,
            Parser::parse_tuple_element_name_or_tuple_element_type,
            ast::Kind::OpenBracketToken,
            ast::Kind::CloseBracketToken,
        );
        finish_node!(self, self.factory.new_tuple_type_node(elements), pos)
    }

    pub fn parse_tuple_element_name_or_tuple_element_type(&mut self) -> ast::Node {
        if self.look_ahead(Parser::scan_start_of_named_tuple_element) {
            let pos = self.node_pos();
            let dot_dot_dot_token = self.parse_optional_token(ast::Kind::DotDotDotToken);
            let name = self.parse_identifier_name();
            let question_token = self.parse_optional_token(ast::Kind::QuestionToken);
            self.parse_expected(ast::Kind::ColonToken);
            let type_node = self.parse_tuple_element_type();
            let result = finish_node!(
                self,
                self.factory.new_named_tuple_member(
                    dot_dot_dot_token,
                    name,
                    question_token,
                    type_node
                ),
                pos
            );
            return result;
        }
        self.parse_tuple_element_type()
    }

    pub fn scan_start_of_named_tuple_element(&mut self) -> bool {
        if self.token == ast::Kind::DotDotDotToken {
            return token_is_identifier_or_keyword(self.next_token())
                && self.next_token_is_colon_or_question_colon();
        }
        token_is_identifier_or_keyword(self.token) && self.next_token_is_colon_or_question_colon()
    }

    pub fn next_token_is_colon_or_question_colon(&mut self) -> bool {
        self.next_token() == ast::Kind::ColonToken
            || self.token == ast::Kind::QuestionToken && self.next_token() == ast::Kind::ColonToken
    }

    pub fn parse_tuple_element_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        if self.parse_optional(ast::Kind::DotDotDotToken) {
            let type_node = self.parse_type();
            return finish_node!(self, self.factory.new_rest_type_node(type_node), pos);
        }
        let type_node = self.parse_type();
        let type_node_loc = self.factory.store().loc(type_node);
        if let Some(question_pos) = self.postfix_question_position(type_node_loc) {
            self.factory.finish_parsed_node_header(
                type_node,
                type_node_loc.with_end(question_pos),
                self.context_flags,
                false,
            );
            let optional_type = self.factory.new_optional_type_node(type_node);
            return self.finish_node_with_end(
                optional_type,
                type_node_loc.pos(),
                type_node_loc.end(),
            );
        }
        if !self.has_preceding_line_break()
            && self.token == ast::Kind::QuestionToken
            && !self.look_ahead(Parser::next_is_start_of_type)
        {
            self.next_token();
            return finish_node!(self, self.factory.new_optional_type_node(type_node), pos);
        }
        type_node
    }

    fn postfix_question_position(&self, loc: core::TextRange) -> Option<i32> {
        let bytes = self.source_text.as_bytes();
        if loc.pos() < 0 || loc.end() <= loc.pos() || loc.end() as usize > bytes.len() {
            return None;
        }
        let question_pos = loc.end() as usize - 1;
        if bytes[question_pos] != b'?' {
            return None;
        }
        let mut end = question_pos;
        while end > loc.pos() as usize && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        Some(end as i32)
    }

    pub fn parse_parenthesized_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenParenToken);
        let type_node = self.parse_type();
        self.parse_expected(ast::Kind::CloseParenToken);
        finish_node!(
            self,
            self.factory.new_parenthesized_type_node(type_node),
            pos
        )
    }

    pub fn parse_literal_type_node(&mut self, negative: bool) -> ast::Node {
        let pos = self.node_pos();
        let literal = if negative {
            self.parse_expected(ast::Kind::MinusToken);
            let expression = self.parse_literal_expression(false);
            finish_node!(
                self,
                self.factory
                    .new_prefix_unary_expression(ast::Kind::MinusToken, expression),
                pos
            )
        } else if self.token == ast::Kind::TrueKeyword
            || self.token == ast::Kind::FalseKeyword
            || self.token == ast::Kind::NullKeyword
        {
            self.parse_token_node()
        } else {
            self.parse_literal_expression(false)
        };
        finish_node!(self, self.factory.new_literal_type_node(literal), pos)
    }

    pub fn parse_import_type(&mut self) -> ast::Node {
        self.source_flags |= ast::NodeFlags::POSSIBLY_CONTAINS_DYNAMIC_IMPORT;
        let pos = self.node_pos();
        let is_type_of = self.parse_optional(ast::Kind::TypeOfKeyword);
        self.parse_expected(ast::Kind::ImportKeyword);
        self.parse_expected(ast::Kind::OpenParenToken);
        let type_node = self.parse_type();
        let attributes = if self.parse_optional(ast::Kind::CommaToken) {
            let open_brace_position = self.scanner.as_ref().expect("scanner").token_start();
            self.parse_expected(ast::Kind::OpenBraceToken);
            let current_token = self.token;
            if current_token == ast::Kind::WithKeyword || current_token == ast::Kind::AssertKeyword
            {
                if current_token == ast::Kind::AssertKeyword {
                    self.parse_error_at_current_token(
                        &diagnostics::IMPORT_ASSERTIONS_HAVE_BEEN_REPLACED_BY_IMPORT_ATTRIBUTES_USE_WITH_INSTEAD_OF_ASSERT,
                        Vec::new(),
                    );
                }
                self.next_token();
            } else {
                self.parse_error_at_current_token(
                    &diagnostics::X_0_EXPECTED,
                    vec![Box::new(scanner::token_to_string(ast::Kind::WithKeyword))],
                );
            }
            self.parse_expected(ast::Kind::ColonToken);
            let attributes = self.parse_import_attributes(current_token, true);
            self.parse_optional(ast::Kind::CommaToken);
            if !self.parse_expected(ast::Kind::CloseBraceToken) {
                self.add_bracket_related_info(
                    open_brace_position,
                    ast::Kind::OpenBraceToken,
                    ast::Kind::CloseBraceToken,
                );
            }
            Some(attributes)
        } else {
            None
        };
        self.parse_expected(ast::Kind::CloseParenToken);
        let qualifier = if self.parse_optional(ast::Kind::DotToken) {
            Some(self.parse_entity_name_of_type_reference())
        } else {
            None
        };
        let type_arguments = self.parse_type_arguments_of_type_reference();
        finish_node!(
            self,
            self.factory.new_import_type_node(
                is_type_of,
                type_node,
                attributes,
                qualifier,
                type_arguments
            ),
            pos
        )
    }

    pub fn parse_function_or_constructor_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers_for_constructor_type();
        let is_constructor_type = self.parse_optional(ast::Kind::NewKeyword);
        let type_parameters = self.parse_type_parameters();
        let parameters = self.parse_parameters(crate::ParseFlags::TYPE);
        let return_type = self.parse_return_type(ast::Kind::EqualsGreaterThanToken, false);
        let result = if is_constructor_type {
            self.factory.new_constructor_type_node(
                modifiers,
                type_parameters,
                parameters,
                return_type,
            )
        } else {
            self.factory
                .new_function_type_node(type_parameters, parameters, return_type)
        };
        let result = self.finish_node(result, pos);
        result
    }

    pub fn is_start_of_function_type_or_constructor_type(&mut self) -> bool {
        self.token == ast::Kind::LessThanToken
            || self.token == ast::Kind::OpenParenToken
                && self.look_ahead(Parser::next_is_unambiguously_start_of_function_type)
            || self.token == ast::Kind::NewKeyword
            || self.token == ast::Kind::AbstractKeyword
                && self.look_ahead(Parser::next_token_is_new_keyword)
    }

    pub fn next_is_unambiguously_start_of_function_type(&mut self) -> bool {
        self.next_token();
        if self.token == ast::Kind::CloseParenToken || self.token == ast::Kind::DotDotDotToken {
            // ( )
            // ( ...
            return true;
        }
        if self.skip_parameter_start() {
            // We successfully skipped modifiers (if any) and an identifier or binding pattern,
            // now see if we have something that indicates a parameter declaration
            if matches!(
                self.token,
                ast::Kind::ColonToken
                    | ast::Kind::CommaToken
                    | ast::Kind::QuestionToken
                    | ast::Kind::EqualsToken
            ) {
                // ( xxx :
                // ( xxx ,
                // ( xxx ?
                // ( xxx =
                return true;
            }
            if self.token == ast::Kind::CloseParenToken
                && self.next_token() == ast::Kind::EqualsGreaterThanToken
            {
                // ( xxx ) =>
                return true;
            }
        }
        false
    }

    pub fn skip_parameter_start(&mut self) -> bool {
        if ast::is_modifier_kind(self.token) {
            // Skip modifiers
            self.parse_modifiers();
        }
        self.parse_optional(ast::Kind::DotDotDotToken);
        if self.is_identifier() || self.token == ast::Kind::ThisKeyword {
            self.next_token();
            return true;
        }
        if self.token == ast::Kind::OpenBracketToken || self.token == ast::Kind::OpenBraceToken {
            // Return true if we can parse an array or object binding pattern with no errors
            let previous_error_count = self.diagnostics.len();
            self.parse_identifier_or_pattern();
            return previous_error_count == self.diagnostics.len();
        }
        false
    }

    pub fn parse_modifiers_for_constructor_type(&mut self) -> Option<ast::ModifierList> {
        if self.token == ast::Kind::AbstractKeyword {
            let pos = self.node_pos();
            let mut modifier = self.factory.new_modifier(self.token);
            self.next_token();
            modifier = self.finish_node(modifier, pos);
            return Some(
                self.new_parser_modifier_list(self.factory.store().loc(modifier), vec![modifier]),
            );
        }
        None
    }

    pub fn next_token_is_new_keyword(&mut self) -> bool {
        self.next_token() == ast::Kind::NewKeyword
    }

    pub fn parse_type_parameters(&mut self) -> Option<ast::NodeList> {
        if self.token == ast::Kind::LessThanToken {
            Some(self.parse_bracketed_list(
                ParsingContext::PCTypeParameters,
                Parser::parse_type_parameter,
                ast::Kind::LessThanToken,
                ast::Kind::GreaterThanToken,
            ))
        } else {
            None
        }
    }

    pub fn parse_type_parameter(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers_ex(false, true, false);
        let name = self.parse_identifier();
        let mut expression = None;
        let constraint = if self.parse_optional(ast::Kind::ExtendsKeyword) {
            if self.is_start_of_type(false) || !self.is_start_of_expression() {
                Some(self.parse_type())
            } else {
                expression = Some(self.parse_unary_expression_or_higher());
                None
            }
        } else {
            None
        };
        let default_type = if self.parse_optional(ast::Kind::EqualsToken) {
            Some(self.parse_type())
        } else {
            None
        };
        finish_node!(
            self,
            self.factory.new_type_parameter_declaration(
                modifiers,
                name,
                constraint,
                expression,
                default_type
            ),
            pos
        )
    }

    pub fn parse_function_or_constructor_type_to_error(
        &mut self,
        is_in_union_type: bool,
        parse_constituent_type: fn(&mut Parser) -> ast::Node,
    ) -> ast::Node {
        if self.is_start_of_function_type_or_constructor_type() {
            let type_node = self.parse_function_or_constructor_type();
            let diagnostic = if self.factory.store().kind(type_node) == ast::Kind::FunctionType {
                if is_in_union_type {
                    &diagnostics::FUNCTION_TYPE_NOTATION_MUST_BE_PARENTHESIZED_WHEN_USED_IN_A_UNION_TYPE
                } else {
                    &diagnostics::FUNCTION_TYPE_NOTATION_MUST_BE_PARENTHESIZED_WHEN_USED_IN_AN_INTERSECTION_TYPE
                }
            } else if is_in_union_type {
                &diagnostics::CONSTRUCTOR_TYPE_NOTATION_MUST_BE_PARENTHESIZED_WHEN_USED_IN_A_UNION_TYPE
            } else {
                &diagnostics::CONSTRUCTOR_TYPE_NOTATION_MUST_BE_PARENTHESIZED_WHEN_USED_IN_AN_INTERSECTION_TYPE
            };
            self.parse_error_at_range(self.factory.store().loc(type_node), diagnostic, Vec::new());
            type_node
        } else {
            parse_constituent_type(self)
        }
    }

    pub fn parse_expression(&mut self) -> ast::Node {
        let save_context_flags = self.context_flags;
        self.context_flags &= !ast::NodeFlags::DECORATOR_CONTEXT;
        let pos = self.node_pos();
        let mut expr = self.parse_assignment_expression_or_higher();
        loop {
            let operator_token = self.parse_optional_token(ast::Kind::CommaToken);
            if operator_token.is_none() {
                break;
            }
            // PORT NOTE: reshaped for borrowck
            let right = self.parse_assignment_expression_or_higher();
            expr = self.make_binary_expression(expr, operator_token.unwrap(), right, pos);
        }
        self.context_flags = save_context_flags;
        expr
    }

    pub fn parse_expression_allow_in(&mut self) -> ast::Node {
        do_in_context(
            self,
            ast::NodeFlags::DISALLOW_IN_CONTEXT,
            false,
            Parser::parse_expression,
        )
    }

    pub fn parse_assignment_expression_or_higher(&mut self) -> ast::Node {
        self.parse_assignment_expression_or_higher_worker(true)
    }

    pub fn parse_assignment_expression_or_higher_worker(
        &mut self,
        allow_return_type_in_arrow_function: bool,
    ) -> ast::Node {
        let pos = self.node_pos();
        if self.is_yield_expression() {
            return self.parse_yield_expression();
        }
        if let Some(arrow_expression) = self
            .try_parse_parenthesized_arrow_function_expression(allow_return_type_in_arrow_function)
        {
            return arrow_expression;
        }
        if let Some(arrow_expression) = self
            .try_parse_async_simple_arrow_function_expression(allow_return_type_in_arrow_function)
        {
            return arrow_expression;
        }
        let expr = self.parse_binary_expression_or_higher(ast::OPERATOR_PRECEDENCE_LOWEST);
        if self.factory.store().kind(expr) == ast::Kind::Identifier
            && self.token == ast::Kind::EqualsGreaterThanToken
        {
            return self.parse_simple_arrow_function_expression(
                pos,
                expr,
                allow_return_type_in_arrow_function,
                None,
            );
        }
        if ast::is_left_hand_side_expression(self.factory.store(), expr)
            && ast::is_assignment_operator(self.rescan_greater_than_token())
        {
            // PORT NOTE: reshaped for borrowck
            let operator_token = self.parse_token_node();
            let right = self
                .parse_assignment_expression_or_higher_worker(allow_return_type_in_arrow_function);
            return self.make_binary_expression(expr, operator_token, right, pos);
        }
        self.parse_conditional_expression_rest(expr, pos, allow_return_type_in_arrow_function)
    }

    pub fn parse_conditional_expression_or_higher(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let left_operand = self.parse_binary_expression_or_higher(ast::OPERATOR_PRECEDENCE_LOWEST);
        self.parse_conditional_expression_rest(left_operand, pos, true)
    }

    pub fn parse_conditional_expression_rest(
        &mut self,
        left_operand: ast::Node,
        pos: i32,
        allow_return_type_in_arrow_function: bool,
    ) -> ast::Node {
        let question_token = self.parse_optional_token(ast::Kind::QuestionToken);
        if question_token.is_none() {
            return left_operand;
        }
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::DISALLOW_IN_CONTEXT, false);
        let true_expression = self.parse_assignment_expression_or_higher_worker(false);
        self.context_flags = save_context_flags;
        let colon_token = self.parse_expected_token(ast::Kind::ColonToken);
        let false_expression = if ast::node_is_present(self.factory.store(), Some(colon_token)) {
            self.parse_assignment_expression_or_higher_worker(allow_return_type_in_arrow_function)
        } else {
            self.create_missing_identifier()
        };
        finish_node!(
            self,
            self.factory.new_conditional_expression(
                left_operand,
                question_token.unwrap(),
                true_expression,
                colon_token,
                false_expression
            ),
            pos
        )
    }

    pub fn parse_binary_expression_or_higher(&mut self, precedence: i32) -> ast::Node {
        let pos = self.node_pos();
        let left_operand = self.parse_unary_expression_or_higher();
        self.parse_binary_expression_rest(precedence, left_operand, pos)
    }

    pub fn parse_binary_expression_rest(
        &mut self,
        precedence: i32,
        mut left_operand: ast::Node,
        pos: i32,
    ) -> ast::Node {
        loop {
            self.rescan_greater_than_token();
            let new_precedence = ast::get_binary_operator_precedence(self.token) as i32;
            let consume_current_operator = if self.token == ast::Kind::AsteriskAsteriskToken {
                new_precedence >= precedence
            } else {
                new_precedence > precedence
            };
            if !consume_current_operator {
                break;
            }
            if self.token == ast::Kind::InKeyword && self.in_disallow_in_context() {
                break;
            }
            if self.token == ast::Kind::AsKeyword || self.token == ast::Kind::SatisfiesKeyword {
                if self.has_preceding_line_break() {
                    break;
                }
                let keyword_kind = self.token;
                self.next_token();
                if keyword_kind == ast::Kind::SatisfiesKeyword {
                    // PORT NOTE: reshaped for borrowck
                    let type_node = self.parse_type();
                    left_operand = self.make_satisfies_expression(left_operand, type_node);
                } else {
                    // PORT NOTE: reshaped for borrowck
                    let type_node = self.parse_type();
                    left_operand = self.make_as_expression(left_operand, type_node);
                }
            } else {
                // PORT NOTE: reshaped for borrowck
                let operator_token = self.parse_token_node();
                let right = self.parse_binary_expression_or_higher(new_precedence);
                left_operand =
                    self.make_binary_expression(left_operand, operator_token, right, pos);
            }
        }
        left_operand
    }

    pub fn make_satisfies_expression(
        &mut self,
        expression: ast::Node,
        type_node: ast::Node,
    ) -> ast::Node {
        // PORT NOTE: reshaped for borrowck
        let expression_node = finish_node!(
            self,
            self.factory
                .new_satisfies_expression(expression.clone(), type_node),
            self.factory.store().loc(expression).pos()
        );
        self.check_js_syntax(expression_node)
    }

    pub fn make_as_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        // PORT NOTE: reshaped for borrowck
        let expression = finish_node!(
            self,
            self.factory.new_as_expression(left.clone(), right),
            self.factory.store().loc(left).pos()
        );
        self.check_js_syntax(expression)
    }

    pub fn make_binary_expression(
        &mut self,
        left: ast::Node,
        operator_token: ast::Node,
        right: ast::Node,
        pos: i32,
    ) -> ast::Node {
        finish_node!(
            self,
            self.factory.new_binary_expression(
                None::<ast::ModifierList>,
                left,
                None::<ast::Node>,
                operator_token,
                right
            ),
            pos
        )
    }

    pub fn is_update_expression(&self) -> bool {
        match self.token {
            ast::Kind::PlusToken
            | ast::Kind::MinusToken
            | ast::Kind::TildeToken
            | ast::Kind::ExclamationToken
            | ast::Kind::DeleteKeyword
            | ast::Kind::TypeOfKeyword
            | ast::Kind::VoidKeyword
            | ast::Kind::AwaitKeyword => false,
            ast::Kind::LessThanToken => self.language_variant == core::LanguageVariant::JSX,
            _ => true,
        }
    }

    pub fn parse_simple_unary_expression(&mut self) -> ast::Node {
        match self.token {
            ast::Kind::PlusToken
            | ast::Kind::MinusToken
            | ast::Kind::TildeToken
            | ast::Kind::ExclamationToken => self.parse_prefix_unary_expression(),
            ast::Kind::DeleteKeyword => self.parse_delete_expression(),
            ast::Kind::TypeOfKeyword => self.parse_type_of_expression(),
            ast::Kind::VoidKeyword => self.parse_void_expression(),
            ast::Kind::LessThanToken if self.language_variant == core::LanguageVariant::JSX => {
                self.parse_jsx_element_or_self_closing_element_or_fragment(true, -1, None, true)
            }
            ast::Kind::LessThanToken => self.parse_type_assertion(),
            ast::Kind::AwaitKeyword => {
                if self.is_await_expression() {
                    self.parse_await_expression()
                } else {
                    self.parse_update_expression()
                }
            }
            _ => self.parse_update_expression(),
        }
    }

    pub fn parse_delete_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.next_token();
        let expression = self.parse_simple_unary_expression();
        finish_node!(self, self.factory.new_delete_expression(expression), pos)
    }

    pub fn parse_void_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.next_token();
        let expression = self.parse_simple_unary_expression();
        finish_node!(self, self.factory.new_void_expression(expression), pos)
    }

    pub fn parse_await_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.next_token();
        let expression = self.parse_simple_unary_expression();
        finish_node!(self, self.factory.new_await_expression(expression), pos)
    }

    pub fn is_await_expression(&mut self) -> bool {
        self.token == ast::Kind::AwaitKeyword
            && (self.in_await_context()
                || self.look_ahead(
                    Parser::next_token_is_identifier_or_keyword_or_literal_on_same_line,
                ))
    }

    pub fn is_parenthesized_arrow_function_expression(&mut self) -> core::Tristate {
        if self.token == ast::Kind::OpenParenToken
            || self.token == ast::Kind::LessThanToken
            || self.token == ast::Kind::AsyncKeyword
        {
            let state = self.mark();
            let result = self.next_is_parenthesized_arrow_function_expression();
            self.rewind(state);
            return result;
        }
        if self.token == ast::Kind::EqualsGreaterThanToken {
            return core::TSTrue;
        }
        core::TSFalse
    }

    pub fn next_is_parenthesized_arrow_function_expression(&mut self) -> core::Tristate {
        if self.token == ast::Kind::AsyncKeyword {
            self.next_token();
            if self.has_preceding_line_break()
                || (self.token != ast::Kind::OpenParenToken
                    && self.token != ast::Kind::LessThanToken)
            {
                return core::TSFalse;
            }
        }
        let first = self.token;
        let second = self.next_token();
        if first == ast::Kind::OpenParenToken {
            if second == ast::Kind::CloseParenToken {
                return match self.next_token() {
                    ast::Kind::EqualsGreaterThanToken
                    | ast::Kind::ColonToken
                    | ast::Kind::OpenBraceToken => core::TSTrue,
                    _ => core::TSFalse,
                };
            }
            if second == ast::Kind::OpenBracketToken || second == ast::Kind::OpenBraceToken {
                return core::TSUnknown;
            }
            if second == ast::Kind::DotDotDotToken {
                return core::TSTrue;
            }
            if ast::is_modifier_kind(second)
                && second != ast::Kind::AsyncKeyword
                && self.look_ahead(Parser::next_token_is_identifier)
            {
                if self.next_token() == ast::Kind::AsKeyword {
                    return core::TSFalse;
                }
                return core::TSTrue;
            }
            if !self.is_identifier() && second != ast::Kind::ThisKeyword {
                return core::TSFalse;
            }
            match self.next_token() {
                ast::Kind::ColonToken => core::TSTrue,
                ast::Kind::QuestionToken => {
                    self.next_token();
                    if matches!(
                        self.token,
                        ast::Kind::ColonToken
                            | ast::Kind::CommaToken
                            | ast::Kind::EqualsToken
                            | ast::Kind::CloseParenToken
                    ) {
                        core::TSTrue
                    } else {
                        core::TSFalse
                    }
                }
                ast::Kind::CommaToken | ast::Kind::EqualsToken | ast::Kind::CloseParenToken => {
                    core::TSUnknown
                }
                _ => core::TSFalse,
            }
        } else {
            if !self.is_identifier() && self.token != ast::Kind::ConstKeyword {
                return core::TSFalse;
            }
            if self.language_variant == core::LanguageVariant::JSX {
                let is_arrow_function_in_jsx = self.look_ahead(|p| {
                    p.parse_optional(ast::Kind::ConstKeyword);
                    let third = p.next_token();
                    if third == ast::Kind::ExtendsKeyword {
                        let fourth = p.next_token();
                        return !matches!(
                            fourth,
                            ast::Kind::EqualsToken
                                | ast::Kind::GreaterThanToken
                                | ast::Kind::SlashToken
                        );
                    }
                    third == ast::Kind::CommaToken || third == ast::Kind::EqualsToken
                });
                if is_arrow_function_in_jsx {
                    return core::TSTrue;
                }
                return core::TSFalse;
            }
            core::TSUnknown
        }
    }

    pub fn try_parse_parenthesized_arrow_function_expression(
        &mut self,
        allow_return_type_in_arrow_function: bool,
    ) -> Option<ast::Node> {
        let tristate = self.is_parenthesized_arrow_function_expression();
        if tristate == core::TSFalse {
            return None;
        }
        if tristate == core::TSTrue {
            return Some(self.parse_parenthesized_arrow_function_expression(true, true));
        }
        let state = self.mark();
        let result = self.parse_possible_parenthesized_arrow_function_expression(
            allow_return_type_in_arrow_function,
        );
        if result.is_none() {
            self.rewind(state);
        }
        result
    }

    pub fn parse_possible_parenthesized_arrow_function_expression(
        &mut self,
        allow_return_type_in_arrow_function: bool,
    ) -> Option<ast::Node> {
        let token_pos = self.scanner.as_ref().expect("scanner").token_start();
        if self.not_parenthesized_arrow.has(&token_pos) {
            return None;
        }
        let result = self.parse_parenthesized_arrow_function_expression(
            false,
            allow_return_type_in_arrow_function,
        );
        if ast::node_is_missing(self.factory.store(), Some(result)) {
            self.not_parenthesized_arrow.add(token_pos);
            return None;
        }
        Some(result)
    }

    pub fn parse_parenthesized_arrow_function_expression(
        &mut self,
        allow_ambiguity: bool,
        allow_return_type_in_arrow_function: bool,
    ) -> ast::Node {
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers_for_arrow_function();
        let is_async = modifier_list_has_async(self.factory.store(), modifiers.as_ref());
        let signature_flags = if is_async {
            crate::ParseFlags::AWAIT
        } else {
            crate::ParseFlags::NONE
        };
        let type_parameters = self.parse_type_parameters();
        let parameters = if !self.parse_expected(ast::Kind::OpenParenToken) {
            if !allow_ambiguity {
                return self.create_missing_identifier();
            }
            self.create_missing_list()
        } else {
            let Some(parameters) = self.parse_parameters_worker(signature_flags, allow_ambiguity)
            else {
                return self.create_missing_identifier();
            };
            if !self.parse_expected(ast::Kind::CloseParenToken) && !allow_ambiguity {
                return self.create_missing_identifier();
            }
            parameters
        };
        let has_return_colon = self.token == ast::Kind::ColonToken;
        let return_type = self.parse_return_type(ast::Kind::ColonToken, false);
        if return_type.is_some_and(|return_type| {
            !allow_ambiguity
                && Self::type_has_arrow_function_blocking_parse_error(
                    self.factory.store(),
                    return_type,
                )
        }) {
            return self.create_missing_identifier();
        }
        if !allow_ambiguity
            && self.token != ast::Kind::EqualsGreaterThanToken
            && self.token != ast::Kind::OpenBraceToken
        {
            return self.create_missing_identifier();
        }
        let last_token = self.token;
        let equals_greater_than_token =
            self.parse_expected_token(ast::Kind::EqualsGreaterThanToken);
        let body = if last_token == ast::Kind::EqualsGreaterThanToken
            || last_token == ast::Kind::OpenBraceToken
        {
            self.parse_arrow_function_expression_body(is_async, allow_return_type_in_arrow_function)
        } else {
            self.parse_identifier()
        };
        if !allow_return_type_in_arrow_function
            && has_return_colon
            && self.token != ast::Kind::ColonToken
        {
            return self.create_missing_identifier();
        }
        let result = finish_node!(
            self,
            self.factory.new_arrow_function(
                modifiers,
                type_parameters,
                parameters,
                return_type,
                None,
                Some(equals_greater_than_token),
                body
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    // If true, we should abort parsing an error function.
    fn type_has_arrow_function_blocking_parse_error(
        store: &ast::AstStore,
        node: ast::Node,
    ) -> bool {
        match store.kind(node) {
            ast::Kind::TypeReference => ast::node_is_missing(store, store.type_name(node)),
            ast::Kind::FunctionType | ast::Kind::ConstructorType => {
                store
                    .parameters(node)
                    .is_some_and(|parameters| parameters.is_missing())
                    || store.type_node(node).is_some_and(|node| {
                        Self::type_has_arrow_function_blocking_parse_error(store, node)
                    })
            }
            ast::Kind::ParenthesizedType => store.type_node(node).is_some_and(|node| {
                Self::type_has_arrow_function_blocking_parse_error(store, node)
            }),
            _ => false,
        }
    }

    pub fn parse_modifiers_for_arrow_function(&mut self) -> Option<ast::ModifierList> {
        if self.token == ast::Kind::AsyncKeyword {
            let pos = self.node_pos();
            self.next_token();
            let modifier = finish_node!(
                self,
                self.factory.new_modifier(ast::Kind::AsyncKeyword),
                pos
            );
            return Some(
                self.new_parser_modifier_list(self.factory.store().loc(modifier), vec![modifier]),
            );
        }
        None
    }

    pub fn try_parse_async_simple_arrow_function_expression(
        &mut self,
        allow_return_type_in_arrow_function: bool,
    ) -> Option<ast::Node> {
        if self.token == ast::Kind::AsyncKeyword
            && self.look_ahead(Parser::next_is_unparenthesized_async_arrow_function)
        {
            let pos = self.node_pos();
            let async_modifier = self.parse_modifiers_for_arrow_function();
            let expr = self.parse_binary_expression_or_higher(ast::OPERATOR_PRECEDENCE_LOWEST);
            return Some(self.parse_simple_arrow_function_expression(
                pos,
                expr,
                allow_return_type_in_arrow_function,
                async_modifier,
            ));
        }
        None
    }

    pub fn next_is_unparenthesized_async_arrow_function(&mut self) -> bool {
        if self.token == ast::Kind::AsyncKeyword {
            self.next_token();
            if self.has_preceding_line_break() || self.token == ast::Kind::EqualsGreaterThanToken {
                return false;
            }
            let expr = self.parse_binary_expression_or_higher(ast::OPERATOR_PRECEDENCE_LOWEST);
            if !self.has_preceding_line_break()
                && self.factory.store().kind(expr) == ast::Kind::Identifier
                && self.token == ast::Kind::EqualsGreaterThanToken
            {
                return true;
            }
        }
        false
    }

    pub fn parse_simple_arrow_function_expression(
        &mut self,
        pos: i32,
        identifier: ast::Node,
        allow_return_type_in_arrow_function: bool,
        async_modifier: Option<ast::ModifierList>,
    ) -> ast::Node {
        let parameter = finish_node!(
            self,
            self.factory.new_parameter_declaration(
                None::<ast::ModifierList>,
                None::<ast::Node>,
                identifier.clone(),
                None::<ast::Node>,
                None::<ast::Node>,
                None::<ast::Node>
            ),
            self.factory.store().loc(identifier).pos()
        );
        let parameters =
            self.new_parser_node_list(self.factory.store().loc(parameter), vec![parameter]);
        let equals_greater_than_token =
            self.parse_expected_token(ast::Kind::EqualsGreaterThanToken);
        let body = self.parse_arrow_function_expression_body(
            async_modifier.is_some(),
            allow_return_type_in_arrow_function,
        );
        let result = finish_node!(
            self,
            self.factory.new_arrow_function(
                async_modifier,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                Some(equals_greater_than_token),
                body
            ),
            pos
        );
        result
    }

    pub fn parse_arrow_function_expression_body(
        &mut self,
        is_async: bool,
        allow_return_type_in_arrow_function: bool,
    ) -> ast::Node {
        if self.token == ast::Kind::OpenBraceToken {
            return self.parse_function_block(if is_async {
                crate::ParseFlags::AWAIT
            } else {
                crate::ParseFlags::NONE
            });
        }
        if self.token != ast::Kind::SemicolonToken
            && self.token != ast::Kind::FunctionKeyword
            && self.token != ast::Kind::ClassKeyword
            && self.is_start_of_statement()
            && !self.is_start_of_expression_statement()
        {
            return self.parse_function_block(
                crate::ParseFlags::IGNORE_MISSING_OPEN_BRACE
                    | if is_async {
                        crate::ParseFlags::AWAIT
                    } else {
                        crate::ParseFlags::NONE
                    },
            );
        }
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::AWAIT_CONTEXT, is_async);
        self.set_context_flags(ast::NodeFlags::YIELD_CONTEXT, false);
        let node =
            self.parse_assignment_expression_or_higher_worker(allow_return_type_in_arrow_function);
        self.context_flags = save_context_flags;
        node
    }

    pub fn is_start_of_expression_statement(&mut self) -> bool {
        self.token != ast::Kind::OpenBraceToken
            && self.token != ast::Kind::FunctionKeyword
            && self.token != ast::Kind::ClassKeyword
            && self.token != ast::Kind::AtToken
            && self.is_start_of_expression()
    }
    pub fn parse_unary_expression_or_higher(&mut self) -> ast::Node {
        if self.is_update_expression() {
            let pos = self.node_pos();
            let update_expression = self.parse_update_expression();
            if self.token == ast::Kind::AsteriskAsteriskToken {
                return self.parse_binary_expression_rest(
                    ast::get_binary_operator_precedence(self.token) as i32,
                    update_expression,
                    pos,
                );
            }
            return update_expression;
        }
        let unary_operator = self.token;
        let simple_unary_expression = self.parse_simple_unary_expression();
        if self.token == ast::Kind::AsteriskAsteriskToken {
            let pos = scanner::skip_trivia(
                &self.source_text,
                self.factory.store().loc(simple_unary_expression).pos() as usize,
            ) as i32;
            let end = self.factory.store().loc(simple_unary_expression).end();
            if self.factory.store().kind(simple_unary_expression)
                == ast::Kind::TypeAssertionExpression
            {
                self.parse_error_at(pos, end, &diagnostics::A_TYPE_ASSERTION_EXPRESSION_IS_NOT_ALLOWED_IN_THE_LEFT_HAND_SIDE_OF_AN_EXPONENTIATION_EXPRESSION_CONSIDER_ENCLOSING_THE_EXPRESSION_IN_PARENTHESES, Vec::new());
            } else {
                self.parse_error_at(
                    pos,
                    end,
                    &diagnostics::AN_UNARY_EXPRESSION_WITH_THE_0_OPERATOR_IS_NOT_ALLOWED_IN_THE_LEFT_HAND_SIDE_OF_AN_EXPONENTIATION_EXPRESSION_CONSIDER_ENCLOSING_THE_EXPRESSION_IN_PARENTHESES,
                    vec![Box::new(scanner::token_to_string(unary_operator))]);
            }
        }
        simple_unary_expression
    }

    pub fn parse_prefix_unary_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let operator = self.token;
        self.next_token();
        let operand = self.parse_simple_unary_expression();
        finish_node!(
            self,
            self.factory.new_prefix_unary_expression(operator, operand),
            pos
        )
    }

    pub fn parse_prefix_update_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let operator = self.token;
        self.next_token();
        let operand = self.parse_unary_expression_or_higher();
        finish_node!(
            self,
            self.factory.new_prefix_unary_expression(operator, operand),
            pos
        )
    }

    pub fn parse_update_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        if self.token == ast::Kind::PlusPlusToken || self.token == ast::Kind::MinusMinusToken {
            let operator = self.token;
            self.next_token();
            // PORT NOTE: reshaped for borrowck
            let operand = self.parse_left_hand_side_expression_or_higher();
            return finish_node!(
                self,
                self.factory.new_prefix_unary_expression(operator, operand),
                pos
            );
        }
        if self.language_variant == core::LanguageVariant::JSX
            && self.token == ast::Kind::LessThanToken
            && self.look_ahead(Parser::next_token_is_identifier_or_keyword_or_greater_than)
        {
            return self
                .parse_jsx_element_or_self_closing_element_or_fragment(true, -1, None, false);
        }
        let mut expression = self.parse_left_hand_side_expression_or_higher();
        if !self.has_preceding_line_break()
            && (self.token == ast::Kind::PlusPlusToken || self.token == ast::Kind::MinusMinusToken)
        {
            let operator = self.token;
            self.next_token();
            expression = finish_node!(
                self,
                self.factory
                    .new_postfix_unary_expression(expression, operator),
                pos
            );
        }
        expression
    }

    pub fn parse_left_hand_side_expression_or_higher(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let expression = if self.token == ast::Kind::ImportKeyword {
            if self.look_ahead(Parser::next_token_is_open_paren_or_less_than) {
                self.source_flags |= ast::NodeFlags::POSSIBLY_CONTAINS_DYNAMIC_IMPORT;
                self.parse_keyword_expression()
            } else if self.look_ahead(Parser::next_token_is_dot) {
                self.next_token();
                self.next_token();
                let name = self.parse_identifier_name();
                let expression = finish_node!(
                    self,
                    self.factory
                        .new_meta_property(ast::Kind::ImportKeyword, name),
                    pos
                );
                if self.factory.store().text_eq(expression, "defer") {
                    if self.token == ast::Kind::OpenParenToken
                        || self.token == ast::Kind::LessThanToken
                    {
                        self.source_flags |= ast::NodeFlags::POSSIBLY_CONTAINS_DYNAMIC_IMPORT;
                    }
                } else {
                    self.source_flags |= ast::NodeFlags::POSSIBLY_CONTAINS_IMPORT_META;
                }
                expression
            } else {
                self.parse_member_expression_or_higher()
            }
        } else if self.token == ast::Kind::SuperKeyword {
            self.parse_super_expression()
        } else if self.token == ast::Kind::NewKeyword {
            self.parse_new_expression()
        } else {
            self.parse_member_expression_or_higher()
        };
        self.parse_call_expression_rest(pos, expression)
    }

    pub fn parse_member_expression_or_higher(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let primary = self.parse_primary_expression();
        self.parse_member_expression_rest(pos, primary, true)
    }

    pub fn parse_member_expression_rest(
        &mut self,
        pos: i32,
        mut expression: ast::Node,
        allow_optional_chain: bool,
    ) -> ast::Node {
        loop {
            let mut question_dot_token = None;
            let is_property_access = if allow_optional_chain
                && self.is_start_of_optional_property_or_element_access_chain()
            {
                question_dot_token = Some(self.parse_expected_token(ast::Kind::QuestionDotToken));
                token_is_identifier_or_keyword(self.token)
            } else {
                self.parse_optional(ast::Kind::DotToken)
            };
            if is_property_access {
                expression =
                    self.parse_property_access_expression_rest(pos, expression, question_dot_token);
                continue;
            }
            if (question_dot_token.is_some() || !self.in_decorator_context())
                && self.parse_optional(ast::Kind::OpenBracketToken)
            {
                expression =
                    self.parse_element_access_expression_rest(pos, expression, question_dot_token);
                continue;
            }
            if self.is_template_start_of_tagged_template() {
                if question_dot_token.is_none()
                    && ast::is_expression_with_type_arguments(self.factory.store(), expression)
                {
                    let original_expression = self
                        .factory
                        .store()
                        .expression(expression)
                        .expect("ExpressionWithTypeArguments.expression");
                    let original_type_arguments =
                        self.factory.parsed_type_arguments_for_update(expression);
                    expression = self.parse_tagged_template_rest(
                        pos,
                        original_expression,
                        question_dot_token,
                        original_type_arguments,
                    );
                    self.unparse_expression_with_type_arguments(
                        original_expression,
                        original_type_arguments,
                        &expression,
                    );
                } else {
                    expression =
                        self.parse_tagged_template_rest(pos, expression, question_dot_token, None);
                }
                continue;
            }
            if question_dot_token.is_none() {
                if self.token == ast::Kind::ExclamationToken && !self.has_preceding_line_break() {
                    self.next_token();
                    // PORT NOTE: reshaped for borrowck
                    let non_null_expression = finish_node!(
                        self,
                        self.factory
                            .new_non_null_expression(expression, ast::NodeFlags::NONE),
                        pos
                    );
                    expression = self.check_js_syntax(non_null_expression);
                    continue;
                }
                let type_arguments = self.try_parse_type_arguments_in_expression();
                if let Some(type_arguments) = type_arguments {
                    expression = finish_node!(
                        self,
                        self.factory
                            .new_expression_with_type_arguments(expression, Some(type_arguments)),
                        pos
                    );
                    continue;
                }
            }
            return expression;
        }
    }

    pub fn is_start_of_optional_property_or_element_access_chain(&mut self) -> bool {
        self.token == ast::Kind::QuestionDotToken
            && self
                .look_ahead(Parser::next_token_is_identifier_or_keyword_or_open_bracket_or_template)
    }

    pub fn next_token_is_identifier_or_keyword_or_open_bracket_or_template(&mut self) -> bool {
        self.next_token();
        token_is_identifier_or_keyword(self.token)
            || self.token == ast::Kind::OpenBracketToken
            || self.is_template_start_of_tagged_template()
    }

    pub fn parse_property_access_expression_rest(
        &mut self,
        pos: i32,
        mut expression: ast::Node,
        question_dot_token: Option<ast::Node>,
    ) -> ast::Node {
        let name = self.parse_right_side_of_dot(true, true, true);
        let is_optional_chain =
            question_dot_token.is_some() || self.try_reparse_optional_chain(&mut expression);
        let property_access = self.factory.new_property_access_expression(
            expression.clone(),
            question_dot_token,
            name.clone(),
            if is_optional_chain {
                ast::NodeFlags::OPTIONAL_CHAIN
            } else {
                ast::NodeFlags::NONE
            },
        );
        if is_optional_chain && ast::is_private_identifier(self.factory.store(), name) {
            let name_loc = self.factory.store().loc(name);
            self.parse_error_at_range(
                self.skip_range_trivia(name_loc),
                &diagnostics::AN_OPTIONAL_CHAIN_CANNOT_CONTAIN_PRIVATE_IDENTIFIERS,
                Vec::new(),
            );
        }
        if ast::is_expression_with_type_arguments(self.factory.store(), expression) {
            if let Some(type_arguments) = self.factory.store().type_arguments(expression) {
                let type_arguments_loc = type_arguments.loc();
                let loc = core::TextRange::new(
                    type_arguments_loc.pos() - 1,
                    scanner::skip_trivia(&self.source_text, type_arguments_loc.end() as usize)
                        as i32
                        + 1,
                );
                self.parse_error_at_range(
                    loc,
                    &diagnostics::AN_INSTANTIATION_EXPRESSION_CANNOT_BE_FOLLOWED_BY_A_PROPERTY_ACCESS,
                    Vec::new());
            }
        }
        self.finish_node(property_access, pos)
    }

    pub fn try_reparse_optional_chain(&mut self, node: &mut ast::Node) -> bool {
        if self.factory.store().flags(*node) & ast::NodeFlags::OPTIONAL_CHAIN
            != ast::NodeFlags::NONE
        {
            return true;
        }
        if ast::is_non_null_expression(self.factory.store(), *node) {
            let mut expr = self
                .factory
                .store()
                .expression(*node)
                .expect("NonNullExpression.expression");
            while ast::is_non_null_expression(self.factory.store(), expr)
                && self.factory.store().flags(expr) & ast::NodeFlags::OPTIONAL_CHAIN
                    == ast::NodeFlags::NONE
            {
                expr = self
                    .factory
                    .store()
                    .expression(expr)
                    .expect("NonNullExpression.expression");
            }
            if self.factory.store().flags(expr) & ast::NodeFlags::OPTIONAL_CHAIN
                != ast::NodeFlags::NONE
            {
                fn mark_non_null_optional_chain(factory: &mut ast::NodeFactory, node: ast::Node) {
                    if ast::is_non_null_expression(factory.store(), node) {
                        factory.mark_parsed_optional_chain(node);
                        if let Some(expr) = factory.store().expression(node) {
                            mark_non_null_optional_chain(factory, expr);
                        }
                    }
                }
                self.factory.mark_parsed_optional_chain(*node);
                mark_non_null_optional_chain(&mut self.factory, *node);
                return true;
            }
        }
        false
    }

    pub fn parse_element_access_expression_rest(
        &mut self,
        pos: i32,
        mut expression: ast::Node,
        question_dot_token: Option<ast::Node>,
    ) -> ast::Node {
        let argument_expression = if self.token == ast::Kind::CloseBracketToken {
            self.parse_error_at(
                self.node_pos(),
                self.node_pos(),
                &diagnostics::AN_ELEMENT_ACCESS_EXPRESSION_SHOULD_TAKE_AN_ARGUMENT,
                Vec::new(),
            );
            self.create_missing_identifier()
        } else {
            let argument = self.parse_expression_allow_in();
            match self.factory.store().kind(argument) {
                ast::Kind::StringLiteral
                | ast::Kind::NoSubstitutionTemplateLiteral
                | ast::Kind::NumericLiteral => {
                    let text = self.factory.store().text(argument);
                    let text = self.intern_identifier(text);
                    match self.factory.store().kind(argument) {
                        ast::Kind::StringLiteral => {
                            self.factory.set_parsed_string_literal_text(argument, text);
                        }
                        ast::Kind::NoSubstitutionTemplateLiteral => {
                            self.factory
                                .set_parsed_no_substitution_template_literal_text(argument, text);
                        }
                        ast::Kind::NumericLiteral => {
                            self.factory.set_parsed_numeric_literal_text(argument, text);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            argument
        };
        self.parse_expected(ast::Kind::CloseBracketToken);
        let is_optional_chain =
            question_dot_token.is_some() || self.try_reparse_optional_chain(&mut expression);
        finish_node!(
            self,
            self.factory.new_element_access_expression(
                expression,
                question_dot_token,
                argument_expression,
                if is_optional_chain {
                    ast::NodeFlags::OPTIONAL_CHAIN
                } else {
                    ast::NodeFlags::NONE
                }
            ),
            pos
        )
    }

    pub fn parse_call_expression_rest(&mut self, pos: i32, mut expression: ast::Node) -> ast::Node {
        loop {
            expression = self.parse_member_expression_rest(pos, expression, true);
            let question_dot_token = self.parse_optional_token(ast::Kind::QuestionDotToken);
            let mut type_arguments = if question_dot_token.is_some() {
                let type_arguments = self.try_parse_type_arguments_in_expression();
                if self.is_template_start_of_tagged_template() {
                    expression = self.parse_tagged_template_rest(
                        pos,
                        expression,
                        question_dot_token,
                        type_arguments,
                    );
                    continue;
                }
                type_arguments
            } else {
                None
            };
            if type_arguments.is_some() || self.token == ast::Kind::OpenParenToken {
                if question_dot_token.is_none()
                    && self.factory.store().kind(expression)
                        == ast::Kind::ExpressionWithTypeArguments
                {
                    type_arguments = self.factory.parsed_type_arguments_for_update(expression);
                    let next_expression = self
                        .factory
                        .store()
                        .expression(expression)
                        .expect("ExpressionWithTypeArguments.expression");
                    expression = next_expression;
                }
                let inner = expression.clone();
                let argument_list = self.parse_argument_list();
                let is_optional_chain = question_dot_token.is_some()
                    || self.try_reparse_optional_chain(&mut expression);
                // PORT NOTE: reshaped for borrowck
                let call_expression = finish_node!(
                    self,
                    self.factory.new_call_expression(
                        expression,
                        question_dot_token,
                        type_arguments.clone(),
                        argument_list,
                        if is_optional_chain {
                            ast::NodeFlags::OPTIONAL_CHAIN
                        } else {
                            ast::NodeFlags::NONE
                        }
                    ),
                    pos
                );
                expression = self.check_js_syntax(call_expression);
                self.unparse_expression_with_type_arguments(inner, type_arguments, &expression);
                continue;
            }
            if let Some(question_dot_token) = question_dot_token {
                self.parse_error_at_current_token(&diagnostics::IDENTIFIER_EXPECTED, Vec::new());
                let name = self.create_missing_identifier();
                expression = finish_node!(
                    self,
                    self.factory.new_property_access_expression(
                        expression,
                        Some(question_dot_token),
                        name,
                        ast::NodeFlags::OPTIONAL_CHAIN
                    ),
                    pos
                );
            }
            break;
        }
        expression
    }

    pub fn unparse_expression_with_type_arguments(
        &mut self,
        expression: ast::Node,
        type_arguments: Option<ast::NodeList>,
        result: &ast::Node,
    ) {
        self.factory.link_parsed_parent(expression, Some(*result));
        if let Some(type_arguments) = type_arguments {
            let arguments = self.factory.parsed_node_list_nodes(type_arguments);
            for argument in arguments {
                self.factory.link_parsed_parent(argument, Some(*result));
            }
        }
    }

    pub fn parse_argument_list(&mut self) -> ast::NodeList {
        self.parse_bracketed_list(
            ParsingContext::PCArgumentExpressions,
            Parser::parse_argument_expression,
            ast::Kind::OpenParenToken,
            ast::Kind::CloseParenToken,
        )
    }

    pub fn parse_argument_expression(&mut self) -> ast::Node {
        do_in_context(
            self,
            ast::NodeFlags::DISALLOW_IN_CONTEXT | ast::NodeFlags::DECORATOR_CONTEXT,
            false,
            Parser::parse_argument_or_array_literal_element,
        )
    }

    pub fn parse_tagged_template_rest(
        &mut self,
        pos: i32,
        tag: ast::Node,
        question_dot_token: Option<ast::Node>,
        type_arguments: Option<ast::NodeList>,
    ) -> ast::Node {
        let template = if self.token == ast::Kind::NoSubstitutionTemplateLiteral {
            self.rescan_template_token(true);
            self.parse_literal_expression(false)
        } else {
            self.parse_template_expression(true)
        };
        let is_optional_chain = question_dot_token.is_some()
            || self.factory.store().flags(tag) & ast::NodeFlags::OPTIONAL_CHAIN
                != ast::NodeFlags::NONE;
        // PORT NOTE: reshaped for borrowck
        let tagged_template = finish_node!(
            self,
            self.factory.new_tagged_template_expression(
                tag,
                question_dot_token,
                type_arguments,
                template,
                if is_optional_chain {
                    ast::NodeFlags::OPTIONAL_CHAIN
                } else {
                    ast::NodeFlags::NONE
                }
            ),
            pos
        );
        self.check_js_syntax(tagged_template)
    }

    pub fn parse_template_expression(&mut self, is_tagged_template: bool) -> ast::Node {
        let pos = self.node_pos();
        let head = self.parse_template_head(is_tagged_template);
        let spans = self.parse_template_spans(is_tagged_template);
        finish_node!(self, self.factory.new_template_expression(head, spans), pos)
    }

    pub fn parse_template_spans(&mut self, is_tagged_template: bool) -> ast::NodeList {
        let pos = self.node_pos();
        let mut list = Vec::new();
        loop {
            let span = self.parse_template_span(is_tagged_template);
            let is_middle = self.factory.store().literal(span).is_some_and(|literal| {
                self.factory.store().kind(literal) == ast::Kind::TemplateMiddle
            });
            list.push(span);
            if !is_middle {
                break;
            }
        }
        self.new_parser_node_list(core::TextRange::new(pos, self.node_pos()), list)
    }

    pub fn parse_template_span(&mut self, is_tagged_template: bool) -> ast::Node {
        let pos = self.node_pos();
        let expression = self.parse_expression_allow_in();
        let literal = self.parse_literal_of_template_span(is_tagged_template);
        finish_node!(
            self,
            self.factory.new_template_span(expression, literal),
            pos
        )
    }

    pub fn parse_template_head(&mut self, is_tagged_template: bool) -> ast::Node {
        if !is_tagged_template
            && self.scanner.as_ref().expect("scanner").token_flags() & ast::TokenFlags::IS_INVALID
                != ast::TokenFlags::NONE
        {
            self.rescan_template_token(false);
        }
        let pos = self.node_pos();
        let result = self.factory.new_template_head(
            self.scanner
                .as_ref()
                .expect("scanner")
                .token_value()
                .to_string(),
            self.get_template_literal_raw_text(2),
            self.scanner.as_ref().expect("scanner").token_flags(),
        );
        self.next_token();
        self.finish_node(result, pos)
    }

    pub fn get_template_literal_raw_text(&self, mut end_length: usize) -> String {
        let token_text = self.scanner.as_ref().expect("scanner").token_text();
        if self.scanner.as_ref().expect("scanner").token_flags() & ast::TokenFlags::UNTERMINATED
            != ast::TokenFlags::NONE
        {
            end_length = 0;
        }
        token_text[1..token_text.len().saturating_sub(end_length)].to_string()
    }

    pub fn parse_literal_of_template_span(&mut self, is_tagged_template: bool) -> ast::Node {
        if self.token == ast::Kind::CloseBraceToken {
            self.rescan_template_token(is_tagged_template);
            return self.parse_template_middle_or_tail();
        }
        self.parse_error_at_current_token(
            &diagnostics::X_0_EXPECTED,
            vec![Box::new(scanner::token_to_string(
                ast::Kind::CloseBraceToken,
            ))],
        );
        let pos = self.node_pos();
        finish_node!(
            self,
            self.factory
                .new_template_tail(String::new(), String::new(), ast::TokenFlags::NONE),
            pos
        )
    }

    pub fn parse_template_middle_or_tail(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let result = if self.token == ast::Kind::TemplateMiddle {
            self.factory.new_template_middle(
                self.scanner
                    .as_ref()
                    .expect("scanner")
                    .token_value()
                    .to_string(),
                self.get_template_literal_raw_text(2),
                self.scanner.as_ref().expect("scanner").token_flags(),
            )
        } else {
            self.factory.new_template_tail(
                self.scanner
                    .as_ref()
                    .expect("scanner")
                    .token_value()
                    .to_string(),
                self.get_template_literal_raw_text(1),
                self.scanner.as_ref().expect("scanner").token_flags(),
            )
        };
        self.next_token();
        self.finish_node(result, pos)
    }

    pub fn parse_template_type(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let head = self.parse_template_head(false);
        let spans = self.parse_template_type_spans();
        finish_node!(
            self,
            self.factory.new_template_literal_type_node(head, spans),
            pos
        )
    }

    pub fn parse_template_type_spans(&mut self) -> ast::NodeList {
        let pos = self.node_pos();
        let mut list = Vec::new();
        loop {
            let span = self.parse_template_type_span();
            let is_middle = self.factory.store().literal(span).is_some_and(|literal| {
                self.factory.store().kind(literal) == ast::Kind::TemplateMiddle
            });
            list.push(span);
            if !is_middle {
                break;
            }
        }
        self.new_parser_node_list(core::TextRange::new(pos, self.node_pos()), list)
    }

    pub fn parse_template_type_span(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let type_node = self.parse_type();
        let literal = self.parse_literal_of_template_span(false);
        finish_node!(
            self,
            self.factory
                .new_template_literal_type_span(type_node, literal),
            pos
        )
    }

    pub fn parse_new_expression(&mut self) -> ast::Node {
        self.parse_new_expression_or_new_dot_target()
    }

    pub fn parse_new_expression_or_new_dot_target(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::NewKeyword);
        if self.parse_optional(ast::Kind::DotToken) {
            let name = self.parse_identifier_name();
            return finish_node!(
                self,
                self.factory.new_meta_property(ast::Kind::NewKeyword, name),
                pos
            );
        }
        let expression_pos = self.node_pos();
        // PORT NOTE: reshaped for borrowck
        let primary = self.parse_primary_expression();
        let mut expression = self.parse_member_expression_rest(expression_pos, primary, false);
        let mut type_arguments = None;
        if self.factory.store().kind(expression) == ast::Kind::ExpressionWithTypeArguments {
            type_arguments = self.factory.parsed_type_arguments_for_update(expression);
            let next_expression = self
                .factory
                .store()
                .expression(expression)
                .expect("ExpressionWithTypeArguments.expression");
            expression = next_expression;
        }
        if self.token == ast::Kind::QuestionDotToken {
            self.parse_error_at_current_token(
                &diagnostics::INVALID_OPTIONAL_CHAIN_FROM_NEW_EXPRESSION_DID_YOU_MEAN_TO_CALL_0,
                vec![Box::new(scanner::get_text_of_node_from_source_text(
                    &self.source_text,
                    self.factory.store().loc(expression),
                    false,
                ))],
            );
        }
        let argument_list = if self.token == ast::Kind::OpenParenToken {
            Some(self.parse_argument_list())
        } else {
            None
        };
        // PORT NOTE: reshaped for borrowck
        let new_expression = finish_node!(
            self,
            self.factory
                .new_new_expression(expression, type_arguments, argument_list),
            pos
        );
        self.check_js_syntax(new_expression)
    }

    pub fn parse_primary_expression(&mut self) -> ast::Node {
        match self.token {
            ast::Kind::NoSubstitutionTemplateLiteral => {
                if self.scanner.as_ref().expect("scanner").token_flags()
                    & ast::TokenFlags::IS_INVALID
                    != ast::TokenFlags::NONE
                {
                    self.rescan_template_token(false);
                }
                self.parse_literal_expression(false)
            }
            ast::Kind::OpenParenToken => self.parse_parenthesized_expression(),
            ast::Kind::OpenBracketToken => self.parse_array_literal_expression(),
            ast::Kind::OpenBraceToken => self.parse_object_literal_expression(),
            ast::Kind::AsyncKeyword => {
                if self.look_ahead(Parser::next_token_is_function_keyword_on_same_line) {
                    self.parse_function_expression()
                } else {
                    self.parse_identifier_with_diagnostic(
                        Some(&diagnostics::EXPRESSION_EXPECTED),
                        None,
                    )
                }
            }
            ast::Kind::AtToken => self.parse_decorated_expression(),
            ast::Kind::FunctionKeyword => self.parse_function_expression(),
            ast::Kind::ClassKeyword => self.parse_class_expression(),
            ast::Kind::NewKeyword => self.parse_new_expression_or_new_dot_target(),
            ast::Kind::SlashToken | ast::Kind::SlashEqualsToken => {
                if self.rescan_slash_token() == ast::Kind::RegularExpressionLiteral {
                    self.parse_literal_expression(false)
                } else {
                    self.parse_identifier_with_diagnostic(
                        Some(&diagnostics::EXPRESSION_EXPECTED),
                        None,
                    )
                }
            }
            ast::Kind::TemplateHead => self.parse_template_expression(false),
            ast::Kind::PrivateIdentifier => self.parse_private_identifier(),
            ast::Kind::ThisKeyword
            | ast::Kind::SuperKeyword
            | ast::Kind::TrueKeyword
            | ast::Kind::FalseKeyword
            | ast::Kind::NullKeyword => self.parse_keyword_expression(),
            ast::Kind::StringLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::RegularExpressionLiteral => self.parse_literal_expression(false),
            _ => {
                self.parse_identifier_with_diagnostic(Some(&diagnostics::EXPRESSION_EXPECTED), None)
            }
        }
    }

    pub fn next_token_is_function_keyword_on_same_line(&mut self) -> bool {
        self.next_token();
        self.token == ast::Kind::FunctionKeyword && !self.has_preceding_line_break()
    }

    pub fn parse_super_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let mut expression = self.parse_keyword_expression();
        if self.token == ast::Kind::LessThanToken {
            let start_pos = self.node_pos();
            let type_arguments = self.try_parse_type_arguments_in_expression();
            if let Some(type_arguments) = type_arguments {
                self.parse_error_at(
                    start_pos,
                    self.node_pos(),
                    &diagnostics::X_SUPER_MAY_NOT_USE_TYPE_ARGUMENTS,
                    Vec::new(),
                );
                if !self.is_template_start_of_tagged_template() {
                    expression = finish_node!(
                        self,
                        self.factory
                            .new_expression_with_type_arguments(expression, Some(type_arguments)),
                        pos
                    );
                }
            }
        }
        if matches!(
            self.token,
            ast::Kind::OpenParenToken | ast::Kind::DotToken | ast::Kind::OpenBracketToken
        ) {
            return expression;
        }
        self.parse_error_at_current_token(
            &diagnostics::X_SUPER_MUST_BE_FOLLOWED_BY_AN_ARGUMENT_LIST_OR_MEMBER_ACCESS,
            Vec::new(),
        );
        let name = self.parse_right_side_of_dot(true, true, true);
        finish_node!(
            self,
            self.factory.new_property_access_expression(
                expression,
                None,
                name,
                ast::NodeFlags::NONE
            ),
            pos
        )
    }

    pub fn is_template_start_of_tagged_template(&self) -> bool {
        self.token == ast::Kind::NoSubstitutionTemplateLiteral
            || self.token == ast::Kind::TemplateHead
    }

    pub fn parse_keyword_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let result = self.factory.new_keyword_expression(self.token);
        self.next_token();
        self.finish_node(result, pos)
    }

    pub fn parse_type_of_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.next_token();
        let expression = self.parse_simple_unary_expression();
        finish_node!(self, self.factory.new_type_of_expression(expression), pos)
    }

    pub fn parse_type_assertion(&mut self) -> ast::Node {
        debug::assert(
            self.language_variant != core::LanguageVariant::JSX,
            Some("Type assertions should never be parsed in JSX.".to_string()),
        );
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::LessThanToken);
        let type_node = self.parse_type();
        self.parse_expected(ast::Kind::GreaterThanToken);
        let expression = self.parse_simple_unary_expression();
        finish_node!(
            self,
            self.factory.new_type_assertion(type_node, expression),
            pos
        )
    }

    pub fn parse_asserts_type_predicate(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let asserts_modifier = self.parse_expected_token(ast::Kind::AssertsKeyword);
        let parameter_name = if self.token == ast::Kind::ThisKeyword {
            self.parse_this_type_node()
        } else {
            self.parse_identifier()
        };
        let type_node = if self.parse_optional(ast::Kind::IsKeyword) {
            Some(self.parse_type())
        } else {
            None
        };
        finish_node!(
            self,
            self.factory
                .new_type_predicate_node(Some(asserts_modifier), parameter_name, type_node),
            pos
        )
    }

    pub fn parse_function_expression(&mut self) -> ast::Node {
        let save_context_flags = self.context_flags;
        self.set_context_flags(ast::NodeFlags::DECORATOR_CONTEXT, false);
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers();
        self.parse_expected(ast::Kind::FunctionKeyword);
        let asterisk_token = self.parse_optional_token(ast::Kind::AsteriskToken);
        let is_generator = asterisk_token.is_some();
        let is_async = modifier_list_has_async(self.factory.store(), modifiers.as_ref());
        let signature_flags = if is_generator {
            crate::ParseFlags::YIELD
        } else {
            crate::ParseFlags::NONE
        } | if is_async {
            crate::ParseFlags::AWAIT
        } else {
            crate::ParseFlags::NONE
        };
        let name = if is_generator && is_async {
            do_in_context(
                self,
                ast::NodeFlags::YIELD_CONTEXT | ast::NodeFlags::AWAIT_CONTEXT,
                true,
                Parser::parse_optional_binding_identifier,
            )
        } else if is_generator {
            do_in_context(
                self,
                ast::NodeFlags::YIELD_CONTEXT,
                true,
                Parser::parse_optional_binding_identifier,
            )
        } else if is_async {
            do_in_context(
                self,
                ast::NodeFlags::AWAIT_CONTEXT,
                true,
                Parser::parse_optional_binding_identifier,
            )
        } else {
            self.parse_optional_binding_identifier()
        };
        let type_parameters = self.parse_type_parameters();
        let parameters = self.parse_parameters(signature_flags);
        let return_type = self.parse_return_type(ast::Kind::ColonToken, false);
        let body = self.parse_function_block(signature_flags);
        self.context_flags = save_context_flags;
        let result = finish_node!(
            self,
            self.factory.new_function_expression(
                modifiers,
                asterisk_token,
                name,
                type_parameters,
                parameters,
                return_type,
                None,
                body
            ),
            pos
        );
        self.check_js_syntax(result)
    }

    pub fn parse_optional_binding_identifier(&mut self) -> Option<ast::Node> {
        if self.is_binding_identifier() {
            return Some(self.parse_binding_identifier());
        }
        None
    }

    pub fn parse_decorated_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers_ex(true, false, false);
        if self.token == ast::Kind::ClassKeyword {
            return self.parse_class_declaration_or_expression(
                pos,
                modifiers,
                ast::Kind::ClassExpression,
            );
        }
        self.parse_error_at(
            self.node_pos(),
            self.node_pos(),
            &diagnostics::EXPRESSION_EXPECTED,
            Vec::new(),
        );
        finish_node!(self, self.factory.new_missing_declaration(modifiers), pos)
    }

    pub fn parse_parenthesized_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenParenToken);
        let expression = self.parse_expression();
        self.parse_expected(ast::Kind::CloseParenToken);
        let result = finish_node!(
            self,
            self.factory.new_parenthesized_expression(expression),
            pos
        );
        result
    }

    pub fn parse_arrow_function_expression(
        &mut self,
        pos: i32,
        modifiers: Option<ast::ModifierList>,
        type_parameters: Option<ast::NodeList>,
    ) -> ast::Node {
        let parameters = if self.token == ast::Kind::OpenParenToken {
            self.parse_parameters(crate::ParseFlags::ARROW_FUNCTION)
        } else {
            self.parse_single_parameter()
        };
        let equals_greater_than_token =
            self.parse_expected_token(ast::Kind::EqualsGreaterThanToken);
        let body = if self.token == ast::Kind::OpenBraceToken {
            self.parse_function_block(crate::ParseFlags::ARROW_FUNCTION)
        } else {
            self.parse_assignment_expression_or_higher()
        };
        // PORT NOTE: reshaped for borrowck
        let arrow_function = finish_node!(
            self,
            self.factory.new_arrow_function(
                modifiers,
                type_parameters,
                parameters,
                None,
                None,
                Some(equals_greater_than_token),
                body
            ),
            pos
        );
        self.check_js_syntax(arrow_function)
    }

    pub fn parse_single_parameter(&mut self) -> ast::NodeList {
        let pos = self.node_pos();
        let identifier = self.parse_identifier();
        let parameter = finish_node!(
            self,
            self.factory.new_parameter_declaration(
                None::<ast::ModifierList>,
                None::<ast::Node>,
                identifier.clone(),
                None::<ast::Node>,
                None::<ast::Node>,
                None::<ast::Node>
            ),
            self.factory.store().loc(identifier).pos()
        );
        self.new_parser_node_list(
            core::TextRange::new(pos, self.factory.store().loc(parameter).end()),
            vec![parameter],
        )
    }

    pub fn is_simple_arrow_function_expression(&mut self) -> bool {
        self.token == ast::Kind::Identifier
            && self.look_ahead(Parser::next_token_is_equals_greater_than_token)
    }

    pub fn next_token_is_equals_greater_than_token(&mut self) -> bool {
        self.next_token() == ast::Kind::EqualsGreaterThanToken
    }

    pub fn is_yield_expression(&mut self) -> bool {
        self.token == ast::Kind::YieldKeyword
            && (self.in_yield_context()
                || self.look_ahead(
                    Parser::next_token_is_identifier_or_keyword_or_literal_on_same_line,
                ))
    }

    pub fn parse_yield_expression(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.next_token();
        let (asterisk_token, expression) = if !self.has_preceding_line_break()
            && (self.token == ast::Kind::AsteriskToken || self.is_start_of_expression())
        {
            (
                self.parse_optional_token(ast::Kind::AsteriskToken),
                Some(self.parse_assignment_expression_or_higher()),
            )
        } else {
            (None, None)
        };
        finish_node!(
            self,
            self.factory
                .new_yield_expression(asterisk_token, expression),
            pos
        )
    }

    pub fn is_assignment_operator(&self) -> bool {
        ast::Kind::FirstAssignment <= self.token && self.token <= ast::Kind::LastAssignment
    }

    pub fn try_parse_type_arguments_in_expression(&mut self) -> Option<ast::NodeList> {
        let state = self.mark();
        if self.context_flags & ast::NodeFlags::JAVASCRIPT_FILE == ast::NodeFlags::NONE
            && self.rescan_less_than_token() == ast::Kind::LessThanToken
        {
            self.next_token();
            let type_arguments =
                self.parse_delimited_list(ParsingContext::PCTypeArguments, Parser::parse_type);
            if self.rescan_greater_than_token() == ast::Kind::GreaterThanToken {
                self.next_token();
                if self.can_follow_type_arguments_in_expression() {
                    return Some(type_arguments);
                }
            }
        }
        self.rewind(state);
        None
    }

    pub fn can_follow_type_arguments_in_expression(&mut self) -> bool {
        match self.token {
            ast::Kind::OpenParenToken
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::TemplateHead => true,
            ast::Kind::LessThanToken
            | ast::Kind::GreaterThanToken
            | ast::Kind::PlusToken
            | ast::Kind::MinusToken => false,
            _ => {
                self.has_preceding_line_break()
                    || self.is_binary_operator()
                    || !self.is_start_of_expression()
            }
        }
    }

    pub fn parse_jsx_element_or_self_closing_element_or_fragment(
        &mut self,
        in_expression_context: bool,
        top_invalid_node_position: i32,
        opening_tag: Option<ast::Node>,
        must_be_unary: bool,
    ) -> ast::Node {
        let pos = self.node_pos();
        let opening = self
            .parse_jsx_opening_or_self_closing_element_or_opening_fragment(in_expression_context);
        let mut result = match self.factory.store().kind(opening) {
            ast::Kind::JsxOpeningElement => {
                let mut children = self.parse_jsx_children(&opening);
                let closing_element;
                let last_child = self.factory.parsed_node_list_last(children);
                if let Some(last_child) = last_child.as_ref()
                    && self.factory.store().kind(*last_child) == ast::Kind::JsxElement
                    && !ast::tag_names_are_equivalent(
                        self.factory.store(),
                        required_tag_name(
                            self.factory.store(),
                            &required_jsx_opening_element(self.factory.store(), last_child),
                        ),
                        required_tag_name(
                            self.factory.store(),
                            &required_jsx_closing_element(self.factory.store(), last_child),
                        ),
                    )
                    && ast::tag_names_are_equivalent(
                        self.factory.store(),
                        required_tag_name(self.factory.store(), &opening),
                        required_tag_name(
                            self.factory.store(),
                            &required_jsx_closing_element(self.factory.store(), last_child),
                        ),
                    )
                {
                    let last_child_children =
                        self.factory.parsed_jsx_children_for_update(*last_child);
                    let last_child_opening =
                        required_jsx_opening_element(self.factory.store(), last_child);
                    let last_child_closing =
                        required_jsx_closing_element(self.factory.store(), last_child);
                    let end = self.factory.parsed_node_list_loc(last_child_children).end();
                    // PORT NOTE: reshaped for borrowck
                    let missing_identifier_node = self.new_identifier(String::new());
                    let missing_identifier =
                        self.finish_node_with_end(missing_identifier_node, end, end);
                    let new_closing_element_node =
                        self.factory.new_jsx_closing_element(missing_identifier);
                    let new_closing_element =
                        self.finish_node_with_end(new_closing_element_node, end, end);
                    let new_last_node = self.factory.new_jsx_element(
                        last_child_opening.clone(),
                        last_child_children.clone(),
                        new_closing_element.clone(),
                    );
                    let new_last = self.finish_node_with_end(
                        new_last_node,
                        self.factory.store().loc(last_child_opening).pos(),
                        end,
                    );
                    if let Some(opening_element) = self.factory.store().opening_element(new_last) {
                        self.factory
                            .link_parsed_parent(opening_element, Some(new_last));
                    }
                    let new_last_children = self.factory.parsed_jsx_children_for_update(new_last);
                    let new_last_children = self.factory.parsed_node_list_nodes(new_last_children);
                    for child in new_last_children {
                        self.factory.link_parsed_parent(child, Some(new_last));
                    }
                    if let Some(closing_element) = self.factory.store().closing_element(new_last) {
                        self.factory
                            .link_parsed_parent(closing_element, Some(new_last));
                    }
                    let children_loc = self.factory.parsed_node_list_loc(children);
                    let mut rewritten_children = self.factory.parsed_node_list_nodes(children);
                    rewritten_children.pop();
                    rewritten_children.push(new_last);
                    children = self.new_parser_node_list(
                        core::TextRange::new(
                            children_loc.pos(),
                            self.factory
                                .store()
                                .loc(*rewritten_children.last().unwrap())
                                .end(),
                        ),
                        rewritten_children,
                    );
                    closing_element = last_child_closing;
                } else {
                    closing_element =
                        self.parse_jsx_closing_element(&opening, in_expression_context);
                    if !ast::tag_names_are_equivalent(
                        self.factory.store(),
                        required_tag_name(self.factory.store(), &opening),
                        required_tag_name(self.factory.store(), &closing_element),
                    ) {
                        if let Some(parent_opening_tag) = opening_tag.as_ref()
                            && ast::is_jsx_opening_element(
                                self.factory.store(),
                                *parent_opening_tag,
                            )
                            && ast::tag_names_are_equivalent(
                                self.factory.store(),
                                required_tag_name(self.factory.store(), &closing_element),
                                required_tag_name(self.factory.store(), parent_opening_tag),
                            )
                        {
                            // opening incorrectly matched with its parent's closing -- put error on opening
                            let opening_tag_name =
                                required_tag_name(self.factory.store(), &opening);
                            self.parse_error_at_range(
                                self.factory.store().loc(opening_tag_name),
                                &diagnostics::JSX_ELEMENT_0_HAS_NO_CORRESPONDING_CLOSING_TAG,
                                vec![Box::new(scanner::get_text_of_node_from_source_text(
                                    &self.source_text,
                                    self.factory.store().loc(opening_tag_name),
                                    false,
                                ))],
                            );
                        } else {
                            // other opening/closing mismatches -- put error on closing
                            let closing_tag_name =
                                required_tag_name(self.factory.store(), &closing_element);
                            let opening_tag_name =
                                required_tag_name(self.factory.store(), &opening);
                            self.parse_error_at_range(
                                self.factory.store().loc(closing_tag_name),
                                &diagnostics::EXPECTED_CORRESPONDING_JSX_CLOSING_TAG_FOR_0,
                                vec![Box::new(scanner::get_text_of_node_from_source_text(
                                    &self.source_text,
                                    self.factory.store().loc(opening_tag_name),
                                    false,
                                ))],
                            );
                        }
                    }
                }
                let result = finish_node!(
                    self,
                    self.factory
                        .new_jsx_element(opening, children, closing_element.clone()),
                    pos
                );
                self.factory
                    .link_parsed_parent(closing_element, Some(result));
                result
            }
            ast::Kind::JsxOpeningFragment => {
                // PORT NOTE: reshaped for borrowck
                let children = self.parse_jsx_children(&opening);
                let closing_fragment = self.parse_jsx_closing_fragment(in_expression_context);
                finish_node!(
                    self,
                    self.factory
                        .new_jsx_fragment(opening.clone(), children, closing_fragment),
                    pos
                )
            }
            ast::Kind::JsxSelfClosingElement => opening,
            _ => panic!("Unhandled case in parseJsxElementOrSelfClosingElementOrFragment"),
        };
        if !must_be_unary && in_expression_context && self.token == ast::Kind::LessThanToken {
            let top_bad_pos = if top_invalid_node_position < 0 {
                self.factory.store().loc(result).pos()
            } else {
                top_invalid_node_position
            };
            let invalid_element = self.parse_jsx_element_or_self_closing_element_or_fragment(
                true,
                top_bad_pos,
                None,
                false,
            );
            let operator_token = self.factory.new_token(ast::Kind::CommaToken);
            let invalid_element_loc = self.factory.store().loc(invalid_element);
            self.factory.finish_parsed_node_header(
                operator_token,
                core::TextRange::new(invalid_element_loc.pos(), invalid_element_loc.pos()),
                self.context_flags,
                false,
            );
            self.parse_error_at(
                scanner::skip_trivia(&self.source_text, top_bad_pos as usize) as i32,
                invalid_element_loc.end(),
                &diagnostics::JSX_EXPRESSIONS_MUST_HAVE_ONE_PARENT_ELEMENT,
                Vec::new(),
            );
            result = finish_node!(
                self,
                self.factory.new_binary_expression(
                    None::<ast::ModifierList>,
                    result,
                    None::<ast::Node>,
                    operator_token,
                    invalid_element
                ),
                pos
            );
        }
        result
    }

    pub fn parse_jsx_children(&mut self, opening_tag: &ast::Node) -> ast::NodeList {
        let pos = self.node_pos();
        let save_parsing_contexts = self.parsing_contexts;
        self.parsing_contexts |= 1 << ParsingContext::PCJsxChildren as i32;
        let mut children = Vec::new();
        loop {
            let current_token = self.rescan_jsx_token(true);
            let child = self.parse_jsx_child(opening_tag, current_token);
            let Some(child) = child else {
                break;
            };
            children.push(child.clone());
            if ast::is_jsx_opening_element(self.factory.store(), *opening_tag)
                && self.factory.store().kind(child) == ast::Kind::JsxElement
                && !ast::tag_names_are_equivalent(
                    self.factory.store(),
                    required_tag_name(
                        self.factory.store(),
                        &required_jsx_opening_element(self.factory.store(), &child),
                    ),
                    required_tag_name(
                        self.factory.store(),
                        &required_jsx_closing_element(self.factory.store(), &child),
                    ),
                )
                && ast::tag_names_are_equivalent(
                    self.factory.store(),
                    required_tag_name(self.factory.store(), opening_tag),
                    required_tag_name(
                        self.factory.store(),
                        &required_jsx_closing_element(self.factory.store(), &child),
                    ),
                )
            {
                break;
            }
        }
        self.parsing_contexts = save_parsing_contexts;
        self.new_parser_node_list(core::TextRange::new(pos, self.node_pos()), children)
    }

    pub fn parse_jsx_child(
        &mut self,
        opening_tag: &ast::Node,
        token: ast::Kind,
    ) -> Option<ast::Node> {
        match token {
            ast::Kind::EndOfFile => {
                if ast::is_jsx_opening_fragment(self.factory.store(), *opening_tag) {
                    self.parse_error_at_range(
                        self.factory.store().loc(*opening_tag),
                        &diagnostics::JSX_FRAGMENT_HAS_NO_CORRESPONDING_CLOSING_TAG,
                        Vec::new(),
                    );
                } else {
                    let tag = required_tag_name(self.factory.store(), opening_tag);
                    let tag_loc = self.factory.store().loc(tag);
                    let start = std::cmp::min(
                        scanner::skip_trivia(&self.source_text, tag_loc.pos() as usize) as i32,
                        tag_loc.end(),
                    );
                    self.parse_error_at(
                        start,
                        tag_loc.end(),
                        &diagnostics::JSX_ELEMENT_0_HAS_NO_CORRESPONDING_CLOSING_TAG,
                        vec![Box::new(scanner::get_text_of_node_from_source_text(
                            &self.source_text,
                            self.factory
                                .store()
                                .loc(required_tag_name(self.factory.store(), opening_tag)),
                            false,
                        ))],
                    );
                }
                None
            }
            ast::Kind::LessThanSlashToken | ast::Kind::ConflictMarkerTrivia => None,
            ast::Kind::JsxText | ast::Kind::JsxTextAllWhiteSpaces => Some(self.parse_jsx_text()),
            ast::Kind::OpenBraceToken => Some(self.parse_jsx_expression(false)),
            ast::Kind::LessThanToken => {
                Some(self.parse_jsx_element_or_self_closing_element_or_fragment(
                    false,
                    -1,
                    Some(opening_tag.clone()),
                    false,
                ))
            }
            _ => panic!("Unhandled case in parseJsxChild"),
        }
    }

    pub fn parse_jsx_text(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let text = self
            .scanner
            .as_ref()
            .expect("scanner")
            .token_value()
            .to_string();
        let kind = self.token;
        self.scan_jsx_text();
        finish_node!(
            self,
            self.factory
                .new_jsx_text(text, kind == ast::Kind::JsxTextAllWhiteSpaces),
            pos
        )
    }

    pub fn parse_jsx_expression(&mut self, in_expression_context: bool) -> ast::Node {
        let pos = self.node_pos();
        if !self.parse_expected(ast::Kind::OpenBraceToken) {
            return self.create_missing_identifier();
        }
        let dot_dot_dot_token =
            if self.token != ast::Kind::CloseBraceToken && !in_expression_context {
                self.parse_optional_token(ast::Kind::DotDotDotToken)
            } else {
                None
            };
        let expression = if self.token != ast::Kind::CloseBraceToken {
            Some(self.parse_expression())
        } else {
            None
        };
        if in_expression_context {
            self.parse_expected(ast::Kind::CloseBraceToken);
        } else if self.parse_expected_without_advancing(ast::Kind::CloseBraceToken) {
            self.scan_jsx_text();
        }
        finish_node!(
            self,
            self.factory
                .new_jsx_expression(dot_dot_dot_token, expression),
            pos
        )
    }

    pub fn parse_jsx_closing_element(
        &mut self,
        open: &ast::Node,
        in_expression_context: bool,
    ) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::LessThanSlashToken);
        let tag_name = self.parse_jsx_element_name();
        if self.parse_expected_with_diagnostic(ast::Kind::GreaterThanToken, None, false) {
            if in_expression_context
                || !ast::tag_names_are_equivalent(
                    self.factory.store(),
                    required_tag_name(self.factory.store(), open),
                    tag_name,
                )
            {
                self.next_token();
            } else {
                self.scan_jsx_text();
            }
        }
        finish_node!(self, self.factory.new_jsx_closing_element(tag_name), pos)
    }

    pub fn parse_jsx_opening_or_self_closing_element_or_opening_fragment(
        &mut self,
        in_expression_context: bool,
    ) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::LessThanToken);
        if self.token == ast::Kind::GreaterThanToken {
            self.scan_jsx_text();
            return finish_node!(self, self.factory.new_jsx_opening_fragment(), pos);
        }
        let tag_name = self.parse_jsx_element_name();
        let type_arguments =
            if self.context_flags & ast::NodeFlags::JAVASCRIPT_FILE == ast::NodeFlags::NONE {
                self.parse_type_arguments()
            } else {
                None
            };
        let attributes = self.parse_jsx_attributes();
        let result = if self.token == ast::Kind::GreaterThanToken {
            self.scan_jsx_text();
            self.factory
                .new_jsx_opening_element(tag_name, type_arguments, attributes)
        } else {
            self.parse_expected(ast::Kind::SlashToken);
            if self.parse_expected_without_advancing(ast::Kind::GreaterThanToken) {
                if in_expression_context {
                    self.next_token();
                } else {
                    self.scan_jsx_text();
                }
            }
            self.factory
                .new_jsx_self_closing_element(tag_name, type_arguments, attributes)
        };
        self.finish_node(result, pos)
    }

    pub fn parse_jsx_element_name(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let initial_expression = self.parse_jsx_tag_name();
        if ast::is_jsx_namespaced_name(self.factory.store(), initial_expression) {
            return initial_expression;
        }
        let mut expression = initial_expression;
        while self.parse_optional(ast::Kind::DotToken) {
            let name = self.parse_right_side_of_dot(true, false, false);
            expression = finish_node!(
                self,
                self.factory.new_property_access_expression(
                    expression,
                    None,
                    name,
                    ast::NodeFlags::NONE
                ),
                pos
            );
        }
        expression
    }

    pub fn parse_jsx_tag_name(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.scan_jsx_identifier();
        let is_this = self.token == ast::Kind::ThisKeyword;
        let tag_name = self.parse_identifier_name_error_on_unicode_escape_sequence();
        if self.parse_optional(ast::Kind::ColonToken) {
            self.scan_jsx_identifier();
            // PORT NOTE: reshaped for borrowck
            let name = self.parse_identifier_name_error_on_unicode_escape_sequence();
            return finish_node!(
                self,
                self.factory.new_jsx_namespaced_name(tag_name, name),
                pos
            );
        }
        if is_this {
            return finish_node!(
                self,
                self.factory.new_keyword_expression(ast::Kind::ThisKeyword),
                pos
            );
        }
        tag_name
    }

    pub fn parse_jsx_attributes(&mut self) -> ast::Node {
        let pos = self.node_pos();
        let properties =
            self.parse_list(ParsingContext::PCJsxAttributes, Parser::parse_jsx_attribute);
        finish_node!(self, self.factory.new_jsx_attributes(properties), pos)
    }

    pub fn parse_jsx_attribute(&mut self) -> ast::Node {
        if self.token == ast::Kind::OpenBraceToken {
            return self.parse_jsx_spread_attribute();
        }
        let pos = self.node_pos();
        let name = self.parse_jsx_attribute_name();
        let initializer = self.parse_jsx_attribute_value();
        finish_node!(self, self.factory.new_jsx_attribute(name, initializer), pos)
    }

    pub fn parse_jsx_spread_attribute(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::OpenBraceToken);
        self.parse_expected(ast::Kind::DotDotDotToken);
        let expression = self.parse_expression();
        self.parse_expected(ast::Kind::CloseBraceToken);
        finish_node!(self, self.factory.new_jsx_spread_attribute(expression), pos)
    }

    pub fn parse_jsx_attribute_name(&mut self) -> ast::Node {
        let pos = self.node_pos();
        self.scan_jsx_identifier();
        let attr_name = self.parse_identifier_name_error_on_unicode_escape_sequence();
        if self.parse_optional(ast::Kind::ColonToken) {
            self.scan_jsx_identifier();
            // PORT NOTE: reshaped for borrowck
            let name = self.parse_identifier_name_error_on_unicode_escape_sequence();
            return finish_node!(
                self,
                self.factory.new_jsx_namespaced_name(attr_name, name),
                pos
            );
        }
        attr_name
    }

    pub fn parse_jsx_attribute_value(&mut self) -> Option<ast::Node> {
        if self.token == ast::Kind::EqualsToken {
            if self.scan_jsx_attribute_value() == ast::Kind::StringLiteral {
                return Some(self.parse_literal_expression(false));
            }
            if self.token == ast::Kind::OpenBraceToken {
                return Some(self.parse_jsx_expression(true));
            }
            if self.token == ast::Kind::LessThanToken {
                return Some(
                    self.parse_jsx_element_or_self_closing_element_or_fragment(
                        true, -1, None, false,
                    ),
                );
            }
            self.parse_error_at_current_token(&diagnostics::X_OR_JSX_ELEMENT_EXPECTED, Vec::new());
        }
        None
    }

    pub fn parse_jsx_closing_fragment(&mut self, in_expression_context: bool) -> ast::Node {
        let pos = self.node_pos();
        self.parse_expected(ast::Kind::LessThanSlashToken);
        if self.parse_expected_with_diagnostic(
            ast::Kind::GreaterThanToken,
            Some(&diagnostics::EXPECTED_CORRESPONDING_CLOSING_TAG_FOR_JSX_FRAGMENT),
            false,
        ) {
            if in_expression_context {
                self.next_token();
            } else {
                self.scan_jsx_text();
            }
        }
        finish_node!(self, self.factory.new_jsx_closing_fragment(), pos)
    }

    pub fn scan_jsx_text(&mut self) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .scan_jsx_token_ex(false);
        self.token
    }

    pub fn scan_jsx_identifier(&mut self) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .scan_jsx_identifier();
        self.token
    }

    pub fn scan_jsx_attribute_value(&mut self) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .scan_jsx_attribute_value();
        self.token
    }

    pub fn rescan_jsx_token(&mut self, allow_multiline_jsx_text: bool) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .rescan_jsx_token(allow_multiline_jsx_text);
        self.token
    }

    pub fn scan_jsx_token(&mut self, allow_multiline_jsx_text: bool) -> ast::Kind {
        self.token = self
            .scanner
            .as_mut()
            .expect("scanner")
            .scan_jsx_token_ex(allow_multiline_jsx_text);
        self.token
    }

    pub fn check_js_syntax<T>(&mut self, node: T) -> T
    where
        T: AsRef<ast::Node>,
    {
        let n = *node.as_ref();
        let n_flags = self.factory.store().flags(n);
        if n_flags & ast::NodeFlags::JAVASCRIPT_FILE == ast::NodeFlags::NONE
            || n_flags & ast::NodeFlags::REPARSED != ast::NodeFlags::NONE
        {
            return node;
        }
        match self.factory.store().kind(n) {
            ast::Kind::Parameter
            | ast::Kind::PropertyDeclaration
            | ast::Kind::MethodDeclaration => {
                if let Some(token) = self.factory.store().question_token(n) {
                    if self.factory.store().flags(token) & ast::NodeFlags::REPARSED
                        == ast::NodeFlags::NONE
                        && ast::is_question_token(self.factory.store(), Some(token))
                    {
                        self.js_error_at_range(
                            self.factory.store().loc(token),
                            &diagnostics::THE_0_MODIFIER_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                            vec![Box::new("?")],
                        );
                    }
                }
                if ast::is_function_like(self.factory.store(), Some(n))
                    && self.factory.store().body(n).is_none()
                {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::SIGNATURE_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        Vec::new(),
                    );
                } else if let Some(type_node) = self.factory.store().type_node(n) {
                    if self.factory.store().flags(type_node) & ast::NodeFlags::REPARSED
                        == ast::NodeFlags::NONE
                    {
                        self.js_error_at_range(
                            self.factory.store().loc(type_node),
                            &diagnostics::TYPE_ANNOTATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                            Vec::new(),
                        );
                    }
                }
            }
            ast::Kind::MethodSignature
            | ast::Kind::Constructor
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::FunctionExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::ArrowFunction
            | ast::Kind::VariableDeclaration
            | ast::Kind::IndexSignature => {
                if ast::is_function_like(self.factory.store(), Some(n))
                    && self.factory.store().body(n).is_none()
                {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::SIGNATURE_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        Vec::new(),
                    );
                } else if let Some(type_node) = self.factory.store().type_node(n) {
                    if self.factory.store().flags(type_node) & ast::NodeFlags::REPARSED
                        == ast::NodeFlags::NONE
                    {
                        self.js_error_at_range(
                            self.factory.store().loc(type_node),
                            &diagnostics::TYPE_ANNOTATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                            Vec::new(),
                        );
                    }
                }
            }
            ast::Kind::ImportEqualsDeclaration => {
                self.js_error_at_range(
                    self.factory.store().loc(n),
                    &diagnostics::X_IMPORT_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                    Vec::new(),
                );
            }
            ast::Kind::ImportDeclaration => {
                let import_clause = self.factory.store().import_clause(n);
                let is_type_only = import_clause
                    .is_some_and(|clause| self.factory.store().is_type_only(clause) == Some(true));
                if is_type_only {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::X_0_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        vec![Box::new("import type")],
                    );
                }
            }
            ast::Kind::ExportDeclaration => {
                if self.factory.store().is_type_only(n) == Some(true) {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::X_0_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        vec![Box::new("export type")],
                    );
                }
            }
            ast::Kind::ImportSpecifier => {
                if self.factory.store().is_type_only(n) == Some(true) {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::X_0_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        vec![Box::new("import...type")],
                    );
                }
            }
            ast::Kind::ExportSpecifier => {
                if self.factory.store().is_type_only(n) == Some(true) {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::X_0_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        vec![Box::new("export...type")],
                    );
                }
            }
            ast::Kind::ExportAssignment => {
                if self.factory.store().is_export_equals(n).unwrap_or(false) {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::X_EXPORT_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        Vec::new(),
                    );
                }
            }
            ast::Kind::HeritageClause => {
                if self.factory.store().token(n) == Some(ast::Kind::ImplementsKeyword) {
                    self.js_error_at_range(
                        self.factory.store().loc(n),
                        &diagnostics::X_IMPLEMENTS_CLAUSES_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        Vec::new(),
                    );
                }
            }
            ast::Kind::TypeAliasDeclaration => self.js_error_at_range(
                self.factory.store().loc(
                    self.factory
                        .store()
                        .name(n)
                        .expect("TypeAliasDeclaration.name"),
                ),
                &diagnostics::TYPE_ALIASES_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                Vec::new(),
            ),
            ast::Kind::EnumDeclaration => self.js_error_at_range(
                self.factory
                    .store()
                    .loc(self.factory.store().name(n).expect("EnumDeclaration.name")),
                &diagnostics::X_0_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                vec![Box::new("enum")],
            ),
            ast::Kind::InterfaceDeclaration => self.js_error_at_range(
                self.factory.store().loc(
                    self.factory
                        .store()
                        .name(n)
                        .expect("InterfaceDeclaration.name"),
                ),
                &diagnostics::X_0_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                vec![Box::new("interface")],
            ),
            ast::Kind::ModuleDeclaration => self.js_error_at_range(
                self.factory.store().loc(
                    self.factory
                        .store()
                        .name(n)
                        .expect("ModuleDeclaration.name"),
                ),
                &diagnostics::X_0_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                vec![Box::new(scanner::token_to_string(
                    self.factory
                        .store()
                        .keyword(n)
                        .expect("ModuleDeclaration.keyword"),
                ))],
            ),
            ast::Kind::NonNullExpression => self.js_error_at_range(
                self.factory.store().loc(n),
                &diagnostics::NON_NULL_ASSERTIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                Vec::new(),
            ),
            ast::Kind::AsExpression => self.js_error_at_range(
                self.factory.store().loc(
                    self.factory
                        .store()
                        .type_node(n)
                        .expect("AsExpression.type"),
                ),
                &diagnostics::TYPE_ASSERTION_EXPRESSIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                Vec::new(),
            ),
            ast::Kind::SatisfiesExpression => self.js_error_at_range(
                self.factory.store().loc(
                    self.factory
                        .store()
                        .type_node(n)
                        .expect("SatisfiesExpression.type"),
                ),
                &diagnostics::TYPE_SATISFACTION_EXPRESSIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                Vec::new(),
            ),
            _ => {}
        }
        self.check_js_decorator_syntax(&n);
        match self.factory.store().kind(n) {
            ast::Kind::ClassDeclaration
            | ast::Kind::ClassExpression
            | ast::Kind::MethodDeclaration
            | ast::Kind::Constructor
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::FunctionExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::ArrowFunction => {
                if let Some(list) = self.factory.store().type_parameters(n) {
                    if list.iter().any(|node| {
                        self.factory.store().flags(node) & ast::NodeFlags::REPARSED
                            == ast::NodeFlags::NONE
                    }) {
                        self.js_error_at_range(list.loc(), &diagnostics::TYPE_PARAMETER_DECLARATIONS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES, Vec::new());
                    }
                }
                for modifier in self.factory.store().modifier_nodes(n) {
                    let modifier_kind = self.factory.store().kind(modifier);
                    if self.factory.store().flags(modifier) & ast::NodeFlags::REPARSED
                        == ast::NodeFlags::NONE
                        && modifier_kind != ast::Kind::Decorator
                        && ast::modifier_to_flag(modifier_kind) & ast::ModifierFlags::JAVASCRIPT
                            == ast::ModifierFlags::NONE
                    {
                        self.js_error_at_range(
                            self.factory.store().loc(modifier),
                            &diagnostics::THE_0_MODIFIER_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                            vec![Box::new(scanner::token_to_string(modifier_kind))],
                        );
                    }
                }
            }
            ast::Kind::VariableStatement | ast::Kind::PropertyDeclaration => {
                for modifier in self.factory.store().modifier_nodes(n) {
                    let modifier_kind = self.factory.store().kind(modifier);
                    if self.factory.store().flags(modifier) & ast::NodeFlags::REPARSED
                        == ast::NodeFlags::NONE
                        && modifier_kind != ast::Kind::Decorator
                        && ast::modifier_to_flag(modifier_kind) & ast::ModifierFlags::JAVASCRIPT
                            == ast::ModifierFlags::NONE
                    {
                        self.js_error_at_range(
                            self.factory.store().loc(modifier),
                            &diagnostics::THE_0_MODIFIER_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                            vec![Box::new(scanner::token_to_string(modifier_kind))],
                        );
                    }
                }
            }
            ast::Kind::Parameter => {
                if self
                    .factory
                    .store()
                    .modifier_nodes(n)
                    .iter()
                    .any(|modifier| ast::is_modifier(self.factory.store(), *modifier))
                {
                    self.js_error_at_range(
                        self.factory
                            .store()
                            .modifiers(n)
                            .expect("Parameter.modifiers")
                            .nodes()
                            .loc(),
                        &diagnostics::PARAMETER_MODIFIERS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                        Vec::new(),
                    );
                }
            }
            ast::Kind::CallExpression
            | ast::Kind::NewExpression
            | ast::Kind::ExpressionWithTypeArguments
            | ast::Kind::JsxSelfClosingElement
            | ast::Kind::JsxOpeningElement
            | ast::Kind::TaggedTemplateExpression => {
                if let Some(list) = self.factory.store().type_arguments(n) {
                    if list.iter().any(|node| {
                        self.factory.store().flags(node) & ast::NodeFlags::REPARSED
                            == ast::NodeFlags::NONE
                    }) {
                        self.js_error_at_range(
                            list.loc(),
                            &diagnostics::TYPE_ARGUMENTS_CAN_ONLY_BE_USED_IN_TYPESCRIPT_FILES,
                            Vec::new(),
                        );
                    }
                }
            }
            _ => {}
        }
        node
    }

    pub fn parse_resolution_mode(
        &mut self,
        mode: &str,
        pos: i32,
        end: i32,
    ) -> core::ResolutionMode {
        if mode == "import" {
            return core::ResolutionMode::ESNext;
        }
        if mode == "require" {
            return core::ResolutionMode::CommonJS;
        }
        self.parse_error_at(
            pos,
            end,
            &diagnostics::X_RESOLUTION_MODE_SHOULD_BE_EITHER_REQUIRE_OR_IMPORT,
            Vec::new(),
        );
        core::ResolutionMode::default()
    }

    pub fn js_error_at_range(
        &mut self,
        loc: core::TextRange,
        message: &diagnostics::Message,
        args: Vec<diagnostics::Argument>,
    ) {
        self.js_diagnostics.push(ast::new_diagnostic(
            None,
            core::TextRange::new(
                scanner::skip_trivia(&self.source_text, loc.pos() as usize) as i32,
                loc.end(),
            ),
            message,
            &args,
        ));
    }

    pub fn check_js_decorator_syntax(&mut self, node: &ast::Node) {
        let modifiers = self.factory.store().modifier_nodes(*node);
        if modifiers.is_empty() {
            return;
        }
        if ast::can_have_illegal_decorators(self.factory.store(), *node) {
            for modifier in modifiers {
                if ast::is_decorator(self.factory.store(), modifier) {
                    self.js_error_at_range(
                        self.factory.store().loc(modifier),
                        &diagnostics::DECORATORS_ARE_NOT_VALID_HERE,
                        Vec::new(),
                    );
                    break;
                }
            }
        } else if ast::can_have_decorators(self.factory.store(), *node) {
            let decorator_index = modifiers
                .iter()
                .position(|modifier| ast::is_decorator(self.factory.store(), *modifier));
            if let Some(decorator_index) = decorator_index {
                if ast::is_class_declaration(self.factory.store(), *node) {
                    let export_index = modifiers
                        .iter()
                        .position(|modifier| is_export_modifier(self.factory.store(), modifier));
                    if let Some(export_index) = export_index {
                        let default_index = modifiers.iter().position(|modifier| {
                            self.factory.store().kind(*modifier) == ast::Kind::DefaultKeyword
                        });
                        if decorator_index > export_index
                            && default_index
                                .is_some_and(|default_index| decorator_index < default_index)
                        {
                            self.js_error_at_range(
                                self.factory.store().loc(modifiers[decorator_index]),
                                &diagnostics::DECORATORS_ARE_NOT_VALID_HERE,
                                Vec::new(),
                            );
                        } else if decorator_index < export_index {
                            let trailing_decorator_index = modifiers[export_index..]
                                .iter()
                                .position(|modifier| {
                                    ast::is_decorator(self.factory.store(), *modifier)
                                })
                                .map(|i| i + export_index);
                            if let Some(trailing_decorator_index) = trailing_decorator_index {
                                let mut diagnostic = ast::new_diagnostic(
                                    None,
                                    core::TextRange::new(
                                        scanner::skip_trivia(
                                            &self.source_text,
                                            self.factory
                                                .store()
                                                .loc(modifiers[trailing_decorator_index])
                                                .pos()
                                                as usize) as i32,
                                        self.factory
                                            .store()
                                            .loc(modifiers[trailing_decorator_index])
                                            .end()),
                                    &diagnostics::DECORATORS_MAY_NOT_APPEAR_AFTER_EXPORT_OR_EXPORT_DEFAULT_IF_THEY_ALSO_APPEAR_BEFORE_EXPORT,
                                    &[]);
                                diagnostic.add_related_info(Some(ast::new_diagnostic(
                                    None,
                                    core::TextRange::new(
                                        scanner::skip_trivia(
                                            &self.source_text,
                                            self.factory
                                                .store()
                                                .loc(modifiers[decorator_index])
                                                .pos()
                                                as usize,
                                        ) as i32,
                                        self.factory.store().loc(modifiers[decorator_index]).end(),
                                    ),
                                    &diagnostics::DECORATOR_USED_BEFORE_EXPORT_HERE,
                                    &[],
                                )));
                                self.js_diagnostics.push(diagnostic);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct ParsedIsolatedEntityName {
    pub store: ast::AstStore,
    pub node: ast::EntityName,
}

pub fn parse_isolated_entity_name(text: &str) -> Option<ParsedIsolatedEntityName> {
    let mut p = get_parser();
    p.initialize_state(
        ast::SourceFileParseOptions::default(),
        Arc::from(text),
        core::ScriptKind::JS,
    );
    p.next_token();
    let node = p.parse_entity_name(true, None);
    if p.token == ast::Kind::EndOfFile && p.diagnostics.is_empty() {
        Some(ParsedIsolatedEntityName {
            store: p.factory.into_store(),
            node,
        })
    } else {
        None
    }
}

pub fn get_error_span_for_node(
    store: &ast::AstStore,
    source_text: &str,
    node: &ast::Node,
) -> core::TextRange {
    let loc = store.loc(*node);
    let pos = scanner::skip_trivia(source_text, loc.pos() as usize) as i32;
    core::TextRange::new(pos, loc.end())
}

pub fn is_double_quoted_string(store: &ast::AstStore, node: &ast::Node) -> bool {
    ast::is_string_literal(store, *node)
        && store
            .token_flags(*node)
            .is_some_and(|flags| flags & ast::TokenFlags::SINGLE_QUOTE == ast::TokenFlags::NONE)
}

#[derive(Default)]
pub struct ParserState {
    pub scanner_state: scanner::ScannerState,
    pub context_flags: ast::NodeFlags,
    pub diagnostics_len: usize,
    pub js_diagnostics_len: usize,
    pub reparsed_clones_len: usize,
    pub statement_has_await_identifier: bool,
    pub has_parse_error: bool,
}

impl Parser {
    pub fn mark(&self) -> ParserState {
        ParserState {
            scanner_state: self.scanner.as_ref().expect("scanner").mark(),
            context_flags: self.context_flags,
            diagnostics_len: self.diagnostics.len(),
            js_diagnostics_len: self.js_diagnostics.len(),
            reparsed_clones_len: self.reparsed_clones.len(),
            statement_has_await_identifier: self.statement_has_await_identifier,
            has_parse_error: self.has_parse_error,
        }
    }

    pub fn rewind(&mut self, state: ParserState) {
        self.scanner
            .as_mut()
            .expect("scanner")
            .rewind(state.scanner_state);
        self.token = self.scanner.as_ref().expect("scanner").token();
        self.context_flags = state.context_flags;
        self.diagnostics.truncate(state.diagnostics_len);
        self.js_diagnostics.truncate(state.js_diagnostics_len);
        self.reparsed_clones.truncate(state.reparsed_clones_len);
        self.statement_has_await_identifier = state.statement_has_await_identifier;
        self.has_parse_error = state.has_parse_error;
    }
}

fn validate_json_value(
    store: &ast::AstStore,
    file_name: &str,
    source_text: &str,
    value_expression: impl Into<Option<ast::Node>>,
    diagnostics_out: &mut Vec<ast::Diagnostic>,
) {
    let Some(value_expression) = value_expression.into() else {
        return;
    };
    match store.kind(value_expression) {
        ast::Kind::TrueKeyword
        | ast::Kind::FalseKeyword
        | ast::Kind::NullKeyword
        | ast::Kind::NumericLiteral => return,
        ast::Kind::StringLiteral => {
            if !is_double_quoted_string(store, &value_expression) {
                diagnostics_out.push(ast::new_diagnostic(
                    None,
                    get_error_span_for_node(store, source_text, &value_expression),
                    &diagnostics::STRING_LITERAL_WITH_DOUBLE_QUOTES_EXPECTED,
                    &[],
                ));
            }
            return;
        }
        ast::Kind::PrefixUnaryExpression => {
            if store
                .operator(value_expression)
                .is_some_and(|operator| operator == ast::Kind::MinusToken)
                && store
                    .operand(value_expression)
                    .is_some_and(|operand| store.kind(operand) == ast::Kind::NumericLiteral)
            {
                return;
            }
        }
        ast::Kind::ObjectLiteralExpression => {
            validate_json_object_literal(
                store,
                file_name,
                source_text,
                value_expression,
                diagnostics_out,
            );
            return;
        }
        ast::Kind::ArrayLiteralExpression => {
            let elements = store
                .elements(value_expression)
                .expect("ArrayLiteralExpression.elements");
            for element in elements {
                validate_json_value(store, file_name, source_text, element, diagnostics_out);
            }
            return;
        }
        _ => {}
    }
    diagnostics_out.push(ast::new_diagnostic(
        None,
        get_error_span_for_node(store, source_text, &value_expression),
        &diagnostics::PROPERTY_VALUE_CAN_ONLY_BE_STRING_LITERAL_NUMERIC_LITERAL_TRUE_FALSE_NULL_OBJECT_LITERAL_OR_ARRAY_LITERAL,
        &[]));
}

fn validate_json_object_literal(
    store: &ast::AstStore,
    file_name: &str,
    source_text: &str,
    node: ast::Node,
    diagnostics_out: &mut Vec<ast::Diagnostic>,
) {
    let properties = store
        .properties(node)
        .expect("ObjectLiteralExpression.properties");
    for element in properties {
        if store.kind(element) != ast::Kind::PropertyAssignment {
            diagnostics_out.push(ast::new_diagnostic(
                None,
                get_error_span_for_node(store, source_text, &element),
                &diagnostics::PROPERTY_ASSIGNMENT_EXPECTED,
                &[],
            ));
            continue;
        }
        if let Some(name) = store.name(element) {
            if !is_double_quoted_string(store, &name) {
                diagnostics_out.push(ast::new_diagnostic(
                    None,
                    get_error_span_for_node(store, source_text, &name),
                    &diagnostics::STRING_LITERAL_WITH_DOUBLE_QUOTES_EXPECTED,
                    &[],
                ));
            }
        }
        let initializer = store
            .initializer(element)
            .expect("PropertyAssignment.initializer");
        validate_json_value(store, file_name, source_text, initializer, diagnostics_out);
    }
}

pub fn get_comment_pragmas(_factory: &mut ast::NodeFactory, source_text: &str) -> Vec<ast::Pragma> {
    let mut pragmas = Vec::new();
    for comment_range in scanner::get_leading_comment_ranges(source_text, 0) {
        let pos = comment_range.text_range.pos() as usize;
        let end = comment_range.text_range.end() as usize;
        pragmas.extend(extract_pragmas(comment_range, &source_text[pos..end]));
    }
    pragmas
}

pub fn extract_pragmas(comment_range: ast::CommentRange, text: &str) -> Vec<ast::Pragma> {
    if comment_range.kind == ast::Kind::SingleLineCommentTrivia {
        let mut pos = 2;
        let triple_slash = text_matches(text, pos, "/");
        if triple_slash {
            pos += 1;
        }
        pos = skip_blanks(text, pos);
        if triple_slash && text_matches(text, pos, "<") {
            let tag_name = extract_name(text, pos + 1);
            if tag_name != "reference" {
                return Vec::new();
            }
            pos += 10;
            let mut args = FastHashMap::new();
            loop {
                pos = skip_blanks(text, pos);
                if text_matches(text, pos, "/>") {
                    break;
                }
                let arg_name = extract_name(text, pos);
                if arg_name.is_empty() {
                    break;
                }
                pos = skip_blanks(text, pos + arg_name.len());
                if !text_matches(text, pos, "=") {
                    break;
                }
                pos = skip_blanks(text, pos + 1);
                let Some(value) = extract_quoted_string(text, pos) else {
                    break;
                };
                args.insert(
                    arg_name.clone(),
                    ast::PragmaArgument {
                        name: arg_name,
                        value: value.clone(),
                        text_range: core::TextRange::new(
                            comment_range.text_range.pos() + pos as i32 + 1,
                            comment_range.text_range.pos() + pos as i32 + 1 + value.len() as i32,
                        ),
                    },
                );
                pos += value.len() + 2;
            }
            return vec![ast::Pragma {
                comment_range,
                name: "reference".to_string(),
                args,
            }];
        }
        if text_matches(text, pos, "@") {
            pos += 1;
            let pragma_name = extract_name(text, pos);
            if pragma_name == "ts-check" || pragma_name == "ts-nocheck" {
                return vec![ast::Pragma {
                    comment_range,
                    name: pragma_name,
                    args: FastHashMap::new(),
                }];
            }
        }
    }
    if comment_range.kind == ast::Kind::MultiLineCommentTrivia {
        let text = text.strip_suffix("*/").unwrap_or(text);
        let mut pos = 2;
        let mut pragmas = Vec::new();
        loop {
            let Some(next) = skip_to(text, pos, "@") else {
                break;
            };
            pos = next;
            let pragma_name = extract_name(text, pos + 1);
            if !matches!(
                pragma_name.as_str(),
                "jsx" | "jsxfrag" | "jsximportsource" | "jsxruntime"
            ) {
                break;
            }
            let start = skip_blanks(text, pos + pragma_name.len() + 1);
            pos = skip_non_blanks(text, start);
            if pos == start {
                break;
            }
            let value = text[start..pos].to_string();
            let mut args = FastHashMap::new();
            args.insert(
                "factory".to_string(),
                ast::PragmaArgument {
                    name: "factory".to_string(),
                    value,
                    text_range: core::TextRange::new(
                        comment_range.text_range.pos() + start as i32,
                        comment_range.text_range.pos() + pos as i32,
                    ),
                },
            );
            pragmas.push(ast::Pragma {
                comment_range: comment_range.clone(),
                name: pragma_name,
                args,
            });
        }
        return pragmas;
    }
    Vec::new()
}

pub fn text_matches(text: &str, pos: usize, s: &str) -> bool {
    text.get(pos..).is_some_and(|text| text.starts_with(s))
}

pub fn skip_blanks(text: &str, mut pos: usize) -> usize {
    while pos < text.len() && matches!(text.as_bytes()[pos], b' ' | b'\t') {
        pos += 1;
    }
    pos
}

pub fn skip_non_blanks(text: &str, mut pos: usize) -> usize {
    while pos < text.len() && !matches!(text.as_bytes()[pos], b' ' | b'\t' | b'\r' | b'\n') {
        pos += 1;
    }
    pos
}

pub fn skip_to(text: &str, pos: usize, s: &str) -> Option<usize> {
    if pos >= text.len() {
        return None;
    }
    text[pos..].find(s).map(|offset| pos + offset)
}

pub fn extract_name(text: &str, mut pos: usize) -> String {
    let start = pos;
    while pos < text.len() {
        let byte = text.as_bytes()[pos];
        if !(byte.is_ascii_alphabetic() || byte == b'-') {
            break;
        }
        pos += 1;
    }
    text[start..pos].to_ascii_lowercase()
}

pub fn extract_quoted_string(text: &str, mut pos: usize) -> Option<String> {
    if pos == text.len() {
        return None;
    }
    let quote = text.as_bytes()[pos];
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    pos += 1;
    let start = pos;
    while pos < text.len() && text.as_bytes()[pos] != quote {
        pos += 1;
    }
    if pos == text.len() {
        return None;
    }
    Some(text[start..pos].to_string())
}
