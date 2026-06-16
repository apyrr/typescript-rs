use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_scanner as scanner;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use super::{
    QuotePreference, UserPreferences, get_last_token_info,
    syntax_requires_trailing_comma_or_semicolon_or_asi, syntax_requires_trailing_semicolon_or_asi,
};

pub fn probably_uses_semicolons(file: &ast::SourceFile) -> bool {
    let mut with_semicolon = 0;
    let mut without_semicolon = 0;
    let n_statements_to_observe = 5;

    fn visit(
        node: ast::Node,
        file: &ast::SourceFile,
        with_semicolon: &mut i32,
        without_semicolon: &mut i32,
        n_statements_to_observe: i32,
    ) -> bool {
        let store = file.store();
        if store.flags(node).contains(ast::NodeFlags::REPARSED) {
            return false;
        }
        if syntax_requires_trailing_semicolon_or_asi(store.kind(node)) {
            let last_token = get_last_token_info(Some(node), file);
            if last_token.is_some_and(|last_token| last_token.kind == ast::Kind::SemicolonToken) {
                *with_semicolon += 1;
            } else {
                *without_semicolon += 1;
            }
        } else if syntax_requires_trailing_comma_or_semicolon_or_asi(store.kind(node)) {
            let last_token = get_last_token_info(Some(node), file);
            if last_token.is_some_and(|last_token| last_token.kind == ast::Kind::SemicolonToken) {
                *with_semicolon += 1;
            } else if last_token.is_some_and(|last_token| last_token.kind != ast::Kind::CommaToken)
            {
                let last_token = last_token.unwrap();
                let last_token_line =
                    scanner::get_ecma_line_of_position(file, last_token.loc.pos());
                let next_token_line = scanner::get_ecma_line_of_position(
                    file,
                    scanner::skip_trivia(file.text(), last_token.loc.end() as usize) as i32,
                );
                // Avoid counting missing semicolon in single-line objects:
                // `function f(p: { x: string /*no semicolon here is insignificant*/ }) {`
                if last_token_line != next_token_line {
                    *without_semicolon += 1;
                }
            }
        }

        if *with_semicolon + *without_semicolon >= n_statements_to_observe {
            return true;
        }

        file.store()
            .for_each_present_child(node, |child| {
                if visit(
                    child,
                    file,
                    with_semicolon,
                    without_semicolon,
                    n_statements_to_observe,
                ) {
                    std::ops::ControlFlow::Break(())
                } else {
                    std::ops::ControlFlow::Continue(())
                }
            })
            .is_break()
    }

    for node in file.statements_view() {
        if visit(
            node,
            file,
            &mut with_semicolon,
            &mut without_semicolon,
            n_statements_to_observe,
        ) {
            break;
        }
    }

    // One statement missing a semicolon isn't sufficient evidence to say the user
    // doesn't want semicolons, because they may not even be done writing that statement.
    if with_semicolon == 0 && without_semicolon <= 1 {
        return true;
    }

    // When both kinds of observation exist, treat the file as using semicolons when the
    // ratio withSemicolon/withoutSemicolon exceeds 1/nStatementsToObserve (real arithmetic),
    // implemented as an integer inequality to avoid truncation.
    if without_semicolon == 0 {
        return true;
    }
    with_semicolon * n_statements_to_observe > without_semicolon
}

pub fn should_use_uri_style_node_core_modules(
    file: &ast::SourceFile,
    program: &compiler::Program,
) -> core::Tristate {
    let store = file.store();
    for node in file.imports() {
        let text = store.text(*node);
        if core::node_core_modules().contains(&text)
            && !core::EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES.contains(&text.as_str())
        {
            if text.starts_with("node:") {
                return core::Tristate::True;
            } else {
                return core::Tristate::False;
            }
        }
    }

    program.uses_uri_style_node_core_modules()
}

pub fn quote_preference_from_string(store: &ast::AstStore, str_: ast::Node) -> QuotePreference {
    if store
        .token_flags(str_)
        .is_some_and(|flags| flags.contains(ast::TOKEN_FLAGS_SINGLE_QUOTE))
    {
        return QuotePreference::Single;
    }
    QuotePreference::Double
}

pub fn get_quote_preference(
    source_file: &ast::SourceFile,
    preferences: &UserPreferences,
) -> QuotePreference {
    if preferences.quote_preference != QuotePreference::Unknown
        && preferences.quote_preference != QuotePreference::Auto
    {
        if preferences.quote_preference == QuotePreference::Single {
            return QuotePreference::Single;
        }
        return QuotePreference::Double;
    }
    // ignore synthetic import added when importHelpers: true
    let store = source_file.store();
    let first_module_specifier = source_file.imports().iter().copied().find(|n| {
        ast::is_string_literal(store, *n)
            && store
                .parent(*n)
                .is_some_and(|parent| !ast::node_is_synthesized(store, parent))
    });
    if let Some(first_module_specifier) = first_module_specifier {
        if ast::is_string_literal(store, first_module_specifier) {
            return quote_preference_from_string(store, first_module_specifier);
        }
    }
    QuotePreference::Double
}

pub fn module_symbol_to_valid_identifier(
    module_symbol_name: &str,
    force_capitalize: bool,
) -> String {
    module_specifier_to_valid_identifier(
        stringutil::strip_quotes(module_symbol_name).to_string(),
        force_capitalize,
    )
}

pub fn module_specifier_to_valid_identifier(
    module_specifier: String,
    force_capitalize: bool,
) -> String {
    fn to_go_upper(ch: char) -> char {
        let mut upper = ch.to_uppercase();
        let first = upper.next().unwrap_or(ch);
        if upper.next().is_some() { ch } else { first }
    }

    let extensionless = tspath::remove_file_extension(&module_specifier);
    let base_name = tspath::get_base_file_name(
        extensionless
            .strip_suffix("/index")
            .unwrap_or(&extensionless),
    );
    let mut res = Vec::new();
    let mut last_char_was_valid = true;
    let base_name_chars = base_name.chars().collect::<Vec<_>>();
    if !base_name_chars.is_empty() && scanner::is_identifier_start(base_name_chars[0]) {
        if force_capitalize {
            res.push(to_go_upper(base_name_chars[0]));
        } else {
            res.push(base_name_chars[0]);
        }
    } else {
        last_char_was_valid = false;
    }

    for ch in base_name_chars.into_iter().skip(1) {
        let is_valid = scanner::is_identifier_part(ch);
        if is_valid {
            if !last_char_was_valid {
                res.push(to_go_upper(ch));
            } else {
                res.push(ch);
            }
        }
        last_char_was_valid = is_valid;
    }

    // Need `"_"` to ensure result isn't empty.
    let res_string = res.into_iter().collect::<String>();
    if !res_string.is_empty()
        && !ast::is_non_contextual_keyword(scanner::string_to_token(&res_string))
    {
        return res_string;
    }
    format!("_{res_string}")
}

pub fn is_non_contextual_keyword_public(token: ast::Kind) -> bool {
    ast::is_non_contextual_keyword(token)
}
