use std::fmt::Write as _;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_scanner as scanner;

use crate::LanguageService;
use crate::format::is_in_comment;

const SYMBOL_FORMAT_FLAGS: checker::SymbolFormatFlags =
    checker::SYMBOL_FORMAT_FLAGS_WRITE_TYPE_PARAMETERS_OR_ARGUMENTS
        | checker::SYMBOL_FORMAT_FLAGS_USE_ONLY_EXTERNAL_ALIASING
        | checker::SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND
        | checker::SYMBOL_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE;
const TYPE_FORMAT_FLAGS: checker::TypeFormatFlags =
    checker::TYPE_FORMAT_FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE
        | checker::TYPE_FORMAT_FLAGS_USE_INSTANTIATION_EXPRESSIONS;

fn symbol_to_string_ex<'a>(
    checker: &mut checker::Checker<'a, '_>,
    symbol: ast::SymbolIdentity,
    enclosing_declaration: Option<ast::Node>,
) -> String {
    let _ = (enclosing_declaration, SYMBOL_FORMAT_FLAGS);
    checker.symbol_name_public(symbol).unwrap_or_default()
}

impl LanguageService<'_> {
    pub fn provide_hover(
        &self,
        ctx: &core::Context,
        params: &lsproto::HoverParams,
    ) -> Result<lsproto::HoverResponse, core::Error> {
        let caps = lsproto::get_client_capabilities(ctx);
        let content_format =
            lsproto::preferred_markup_kind(&caps.text_document.hover.content_format);

        let verbosity_level = params.verbosity_level.map_or(0, |level| level as i32);
        let (program, file) = self.get_program_and_file(params.text_document.uri.to_string());
        let position = self
            .converters
            .line_and_character_to_position(file, params.position) as i32;
        let Some(node) = astnav::get_touching_property_name(file, position) else {
            return Ok(lsproto::HoverOrNull::default());
        };
        let store = file.store();
        if ast::is_source_file(store, node)
            || ast::is_property_access_or_qualified_name(store, node)
                && is_in_comment(file, position, Some(&node)).is_none()
        {
            return Ok(lsproto::HoverOrNull::default());
        }

        program.with_type_checker_for_file_using(compiler::CheckerAccess::context(ctx), file, |checker| {
            let range_node = get_node_for_quick_info(node);
            let symbol = get_symbol_at_location_for_quick_info(checker, node);
            let mut max_trunc_len = self.user_preferences().maximum_hover_length;
            if max_trunc_len <= 0 {
                max_trunc_len = 500;
            }
            let mut vc = checker::VerbosityContext {
                level: verbosity_level,
                max_truncation_length: max_trunc_len,
                can_increase_verbosity: false,
                truncated: false,
            };

            let (quick_info, documentation) = self.get_quick_info_and_documentation_for_symbol(
                checker,
                symbol,
                range_node,
                content_format.clone(),
                &mut vc,
            );
            if quick_info.is_empty() {
                return Ok(lsproto::HoverOrNull::default());
            }
            let hover_range = self.get_lsp_range_of_node(range_node, file, None);
            let content = if content_format == lsproto::MarkupKind::Markdown {
                format!("{}{}", format_quick_info(&quick_info), documentation)
            } else {
                format!("{quick_info}{documentation}")
            };
            Ok(lsproto::HoverOrNull {
                hover: Some(lsproto::Hover {
                    contents: lsproto::MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings {
                        markup_content: Some(lsproto::MarkupContent {
                            kind: content_format,
                            value: content,
                        }),
                        ..Default::default()
                    },
                    range: Some(hover_range),
                    can_increase_verbosity: caps.text_document.hover.verbosity_level
                        && vc.can_increase_verbosity
                        && !vc.truncated,
                }),
            })
        })
    }

    pub(crate) fn get_quick_info_and_documentation_for_symbol<'a>(
        &self,
        checker: &mut checker::Checker<'a, '_>,
        symbol: Option<ast::SymbolIdentity>,
        node: ast::Node,
        content_format: lsproto::MarkupKind,
        vc: &mut checker::VerbosityContext,
    ) -> (String, String) {
        let (quick_info, declaration) =
            get_quick_info_and_declaration_at_location(checker, symbol.clone(), node, vc);
        if quick_info.is_empty() {
            return (String::new(), String::new());
        }
        let documentation = self.get_documentation_from_declaration(
            checker,
            symbol,
            declaration,
            node,
            content_format,
            false,
        );
        (quick_info, documentation)
    }

    pub(crate) fn documentation_from_signature<'a>(
        &self,
        _checker: &mut checker::Checker<'a, '_>,
        _symbol: Option<ast::SymbolIdentity>,
        _node: Option<ast::Node>,
        _location: ast::Node,
        _content_format: lsproto::MarkupKind,
        _comment_only: bool,
    ) -> String {
        String::new()
    }

    pub(crate) fn documentation_from_alias<'a>(
        &self,
        _checker: &mut checker::Checker<'a, '_>,
        _symbol: Option<ast::SymbolIdentity>,
        _node: ast::Node,
        _content_format: lsproto::MarkupKind,
    ) -> String {
        String::new()
    }

    pub(crate) fn get_documentation_from_declaration<'a>(
        &self,
        _checker: &mut checker::Checker<'a, '_>,
        _symbol: Option<ast::SymbolIdentity>,
        _declaration: Option<ast::Node>,
        _location: ast::Node,
        _content_format: lsproto::MarkupKind,
        _comment_only: bool,
    ) -> String {
        String::new()
    }

    pub(crate) fn write_comments<'a>(
        &self,
        _output: &mut String,
        _checker: &mut checker::Checker<'a, '_>,
        _comments: &[ast::Node],
        _is_markdown: bool,
    ) {
    }

    pub(crate) fn write_name_link<'a>(
        &self,
        output: &mut String,
        _checker: &mut checker::Checker<'a, '_>,
        _name: ast::Node,
        text: &str,
        quote: bool,
        is_markdown: bool,
    ) {
        write_quoted_string(output, text, quote && is_markdown);
    }
}

pub(crate) fn get_comment_text(_comments: &[ast::Node]) -> String {
    String::new()
}

pub(crate) fn format_quick_info(quick_info: &str) -> String {
    let mut output = String::with_capacity(32);
    write_code(&mut output, "typescript", quick_info);
    output
}

pub(crate) fn should_get_type(_node: ast::Node) -> bool {
    true
}

pub(crate) fn get_quick_info_and_declaration_at_location<'a>(
    checker: &mut checker::Checker<'a, '_>,
    symbol: Option<ast::SymbolIdentity>,
    node: ast::Node,
    vc: &mut checker::VerbosityContext,
) -> (String, Option<ast::Node>) {
    if let Some(symbol) = symbol {
        let quick_info = symbol_to_string_ex(checker, symbol, None);
        if !quick_info.is_empty() {
            return (quick_info, None);
        }
    }
    let t = checker.get_type_at_location(node);
    (
        checker.type_to_string_ex_public(
            t,
            None,
            TYPE_FORMAT_FLAGS | checker::TYPE_FORMAT_FLAGS_MULTILINE_OBJECT_LITERALS,
            Some(vc),
        ),
        None,
    )
}

pub(crate) fn type_parameter_to_string<'a>(
    checker: &mut checker::Checker<'a, '_>,
    t: checker::TypeHandle,
    enclosing_declaration: Option<ast::Node>,
    vc: &mut checker::VerbosityContext,
) -> String {
    checker.type_parameter_to_string_ex(t, enclosing_declaration, Some(vc))
}

pub(crate) fn get_node_for_quick_info(node: ast::Node) -> ast::Node {
    node
}

pub(crate) fn get_symbol_at_location_for_quick_info<'a>(
    checker: &mut checker::Checker<'a, '_>,
    node: ast::Node,
) -> Option<ast::SymbolIdentity> {
    checker.get_symbol_at_location_public(node)
}

pub(crate) fn get_signatures_at_location<'a>(
    checker: &mut checker::Checker<'a, '_>,
    symbol: ast::SymbolIdentity,
    kind: checker::SignatureKind,
    _node: ast::Node,
) -> Vec<checker::SignatureHandle> {
    let Some(symbol_type) = checker.get_type_of_symbol_identity_public(symbol) else {
        return Vec::new();
    };
    let symbol_type = checker.remove_missing_or_undefined_type_public(symbol_type);
    checker.get_signatures_of_type_public(symbol_type, kind)
}

pub(crate) fn get_call_or_new_expression(_node: ast::Node) -> Option<ast::Node> {
    None
}

pub(crate) fn is_node_with_name(_node: ast::Node, _name: &str) -> bool {
    false
}

pub(crate) fn write_code(output: &mut String, lang: &str, code: &str) {
    if code.is_empty() {
        return;
    }
    let mut ticks = 3;
    while code.contains(&"`".repeat(ticks)) {
        ticks += 1;
    }
    let _ = writeln!(
        output,
        "{}{}\n{}\n{}",
        "`".repeat(ticks),
        lang,
        code,
        "`".repeat(ticks)
    );
}

pub(crate) fn trim_comment_prefix(text: &str) -> &str {
    text.trim_start()
        .strip_prefix('|')
        .unwrap_or(text.trim_start())
        .trim_start()
}

pub(crate) fn write_markdown_link(output: &mut String, text: &str, uri: &str, quote: bool) {
    output.push('[');
    write_quoted_string(output, text, quote);
    output.push_str("](");
    output.push_str(uri);
    output.push(')');
}

pub(crate) fn write_optional_entity_name(
    output: &mut String,
    store: &ast::AstStore,
    name: Option<ast::Node>,
) {
    if let Some(name) = name {
        output.push(' ');
        write_quoted_string(output, &get_entity_name_string(store, name), true);
    }
}

pub(crate) fn write_quoted_string(output: &mut String, string: &str, quote: bool) {
    if quote && !string.contains('`') {
        output.push('`');
        output.push_str(string);
        output.push('`');
    } else {
        output.push_str(string);
    }
}

pub(crate) fn find_property_in_type<'a>(
    checker: &mut checker::Checker<'a, '_>,
    object_type: checker::TypeHandle,
    property_name: &str,
) -> Option<ast::SymbolIdentity> {
    if checker.is_union_type_public(object_type) {
        for t in checker.type_types_public(object_type) {
            if let Some(prop) = checker.get_property_of_type_public(t, property_name) {
                return Some(prop);
            }
        }
        return None;
    }
    checker.get_property_of_type_public(object_type, property_name)
}

pub(crate) fn get_entity_name_string(store: &ast::AstStore, name: ast::Node) -> String {
    scanner::token_to_string(store.kind(name))
}

pub(crate) fn write_entity_name_parts(output: &mut String, store: &ast::AstStore, node: ast::Node) {
    output.push_str(&get_entity_name_string(store, node));
}

pub(crate) fn get_container_node(_node: ast::Node) -> Option<ast::Node> {
    None
}

pub(crate) fn get_containing_object_literal_element(_node: ast::Node) -> Option<ast::Node> {
    None
}

pub(crate) fn get_containing_object_literal_element_worker(_node: ast::Node) -> Option<ast::Node> {
    None
}

pub(crate) fn is_object_literal_or_jsx_element(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_object_literal_element(store, &node)
        || ast::is_jsx_attribute(store, node)
        || ast::is_jsx_spread_attribute(store, node)
}

pub(crate) fn get_meaning_from_location(_node: ast::Node) -> ast::SemanticMeaning {
    ast::SemanticMeaning::VALUE
}

pub(crate) fn get_adjusted_location(
    node: ast::Node,
    _for_rename: bool,
    _source_file: Option<&ast::SourceFile>,
) -> ast::Node {
    node
}

pub(crate) fn is_in_right_side_of_internal_import_equals_declaration(_node: ast::Node) -> bool {
    false
}

pub(crate) fn create_range_from_node(node: ast::Node, file: &ast::SourceFile) -> core::TextRange {
    core::new_text_range(
        astnav::get_start_of_node(node, file),
        file.store().loc(node).end(),
    )
}
