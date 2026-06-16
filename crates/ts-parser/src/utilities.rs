use ts_ast as ast;
use ts_core as core;

pub fn get_language_variant(script_kind: core::ScriptKind) -> core::LanguageVariant {
    match script_kind {
        core::ScriptKind::TSX
        | core::ScriptKind::JSX
        | core::ScriptKind::JS
        | core::ScriptKind::JSON => {
            // .tsx and .jsx files are treated as jsx language variant.
            core::LanguageVariant::JSX
        }
        _ => core::LanguageVariant::Standard,
    }
}

pub fn token_is_identifier_or_keyword(token: ast::Kind) -> bool {
    token >= ast::Kind::Identifier
}

pub fn token_is_identifier_or_keyword_or_greater_than(token: ast::Kind) -> bool {
    token == ast::Kind::GreaterThanToken || token_is_identifier_or_keyword(token)
}

pub fn is_keyword_or_punctuation(token: ast::Kind) -> bool {
    ast::is_keyword_kind(token) || ast::is_punctuation_kind(token)
}
