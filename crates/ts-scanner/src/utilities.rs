use ts_ast as ast;
use ts_core as core;

use crate::scanner::{is_identifier_part_ex, is_identifier_start, skip_trivia};

pub const SURR1: u32 = 0xd800;
pub const SURR2: u32 = 0xdc00;
pub const SURR3: u32 = 0xe000;
pub const SURR_SELF: u32 = 0x10000;

pub fn code_point_is_high_surrogate(r: u32) -> bool {
    (SURR1..SURR2).contains(&r)
}

pub fn code_point_is_low_surrogate(r: u32) -> bool {
    (SURR2..SURR3).contains(&r)
}

pub fn surrogate_pair_to_codepoint(r1: u32, r2: u32) -> u32 {
    ((r1 - SURR1) << 10) | ((r2 - SURR2) + SURR_SELF)
}

// encode_surrogate encodes a surrogate code unit (0xD800-0xDFFF) as a 3-byte
// CESU-8 sequence. Standard UTF-8 decoders reject this range, so it acts as a
// sentinel that decode_class_atom_rune can identify when comparing class ranges in
// non-unicode regex mode (where surrogates are valid individual characters).
pub fn encode_surrogate(r: u32) -> Vec<u8> {
    vec![
        0xED,
        (0x80 | ((r >> 6) & 0x3F)) as u8,
        (0x80 | (r & 0x3F)) as u8,
    ]
}

// decode_class_atom_rune is like utf8.DecodeRuneInString but also handles
// surrogate code units encoded by encode_surrogate.
pub fn decode_class_atom_rune(s: &[u8]) -> (u32, usize) {
    let bytes = s;
    if bytes.len() >= 3
        && bytes[0] == 0xED
        && (0xA0..=0xBF).contains(&bytes[1])
        && (0x80..=0xBF).contains(&bytes[2])
    {
        let r = 0xD000 | (((bytes[1] & 0x3F) as u32) << 6) | ((bytes[2] & 0x3F) as u32);
        return (r, 3);
    }
    if bytes.is_empty() {
        return (char::REPLACEMENT_CHARACTER as u32, 0);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        let ch = text.chars().next().unwrap_or(char::REPLACEMENT_CHARACTER);
        return (ch as u32, ch.len_utf8());
    }
    match std::str::from_utf8(bytes) {
        Ok(_) => unreachable!(),
        Err(err) if err.valid_up_to() == 0 => (char::REPLACEMENT_CHARACTER as u32, 1),
        Err(err) => {
            let text = std::str::from_utf8(&bytes[..err.valid_up_to()]).unwrap_or("");
            let ch = text.chars().next().unwrap_or(char::REPLACEMENT_CHARACTER);
            (ch as u32, ch.len_utf8())
        }
    }
}

pub fn token_is_identifier_or_keyword(token: ast::Kind) -> bool {
    token >= ast::Kind::Identifier
}

pub fn identifier_to_keyword_kind(store: &ast::AstStore, node: ast::Node) -> ast::Kind {
    string_to_keyword_kind(&store.text(node))
}

pub fn get_source_text_of_node_from_source_file(
    source_file: &impl ast::SourceFileStoreLike,
    node: &ast::Node,
    include_trivia: bool,
) -> String {
    get_text_of_node_from_source_text(
        source_file.data().text(),
        source_file.store().loc(*node),
        include_trivia,
    )
}

pub fn get_source_text_of_node_from_source_text(
    source_text: &str,
    text_range: core::TextRange,
) -> String {
    source_text[text_range.pos() as usize..text_range.end() as usize].to_string()
}

pub fn get_text_of_node_from_source_text(
    source_text: &str,
    text_range: core::TextRange,
    include_trivia: bool,
) -> String {
    if text_range.pos() == text_range.end() {
        return String::new();
    }
    let mut pos = text_range.pos();
    if !include_trivia {
        pos = skip_trivia(source_text, pos as usize) as _;
    }
    let text = &source_text[pos as usize..text_range.end() as usize];
    text.to_string()
}

pub fn get_text_of_node(source_file: &impl ast::SourceFileStoreLike, node: &ast::Node) -> String {
    get_source_text_of_node_from_source_file(source_file, node, false)
}

pub fn declaration_name_to_string(
    source_file: &impl ast::SourceFileStoreLike,
    name: Option<&ast::Node>,
) -> String {
    let Some(name) = name else {
        return "(Missing)".to_string();
    };
    let loc = source_file.store().loc(*name);
    if loc.pos() == loc.end() {
        return "(Missing)".to_string();
    }
    get_text_of_node(source_file, name)
}

pub fn is_identifier_text(name: &str, language_variant: core::LanguageVariant) -> bool {
    if name.starts_with(ast::INTERNAL_SYMBOL_NAME_PREFIX) {
        return false;
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !is_identifier_start(first) {
        return false;
    }
    chars.all(|ch| is_identifier_part_ex(ch, language_variant))
}

pub fn is_intrinsic_jsx_name(name: &str) -> bool {
    !name.is_empty()
        && (name.as_bytes()[0].is_ascii_lowercase() || name.chars().any(|ch| ch == '-'))
}

pub(crate) fn string_to_keyword_kind(text: &str) -> ast::Kind {
    match text.as_bytes() {
        b"break" => ast::Kind::BreakKeyword,
        b"case" => ast::Kind::CaseKeyword,
        b"catch" => ast::Kind::CatchKeyword,
        b"class" => ast::Kind::ClassKeyword,
        b"const" => ast::Kind::ConstKeyword,
        b"continue" => ast::Kind::ContinueKeyword,
        b"debugger" => ast::Kind::DebuggerKeyword,
        b"default" => ast::Kind::DefaultKeyword,
        b"delete" => ast::Kind::DeleteKeyword,
        b"do" => ast::Kind::DoKeyword,
        b"else" => ast::Kind::ElseKeyword,
        b"enum" => ast::Kind::EnumKeyword,
        b"export" => ast::Kind::ExportKeyword,
        b"extends" => ast::Kind::ExtendsKeyword,
        b"false" => ast::Kind::FalseKeyword,
        b"finally" => ast::Kind::FinallyKeyword,
        b"for" => ast::Kind::ForKeyword,
        b"function" => ast::Kind::FunctionKeyword,
        b"if" => ast::Kind::IfKeyword,
        b"import" => ast::Kind::ImportKeyword,
        b"in" => ast::Kind::InKeyword,
        b"instanceof" => ast::Kind::InstanceOfKeyword,
        b"new" => ast::Kind::NewKeyword,
        b"null" => ast::Kind::NullKeyword,
        b"return" => ast::Kind::ReturnKeyword,
        b"super" => ast::Kind::SuperKeyword,
        b"switch" => ast::Kind::SwitchKeyword,
        b"this" => ast::Kind::ThisKeyword,
        b"throw" => ast::Kind::ThrowKeyword,
        b"true" => ast::Kind::TrueKeyword,
        b"try" => ast::Kind::TryKeyword,
        b"typeof" => ast::Kind::TypeOfKeyword,
        b"var" => ast::Kind::VarKeyword,
        b"void" => ast::Kind::VoidKeyword,
        b"while" => ast::Kind::WhileKeyword,
        b"with" => ast::Kind::WithKeyword,
        b"implements" => ast::Kind::ImplementsKeyword,
        b"interface" => ast::Kind::InterfaceKeyword,
        b"let" => ast::Kind::LetKeyword,
        b"package" => ast::Kind::PackageKeyword,
        b"private" => ast::Kind::PrivateKeyword,
        b"protected" => ast::Kind::ProtectedKeyword,
        b"public" => ast::Kind::PublicKeyword,
        b"static" => ast::Kind::StaticKeyword,
        b"yield" => ast::Kind::YieldKeyword,
        b"abstract" => ast::Kind::AbstractKeyword,
        b"accessor" => ast::Kind::AccessorKeyword,
        b"as" => ast::Kind::AsKeyword,
        b"asserts" => ast::Kind::AssertsKeyword,
        b"assert" => ast::Kind::AssertKeyword,
        b"any" => ast::Kind::AnyKeyword,
        b"async" => ast::Kind::AsyncKeyword,
        b"await" => ast::Kind::AwaitKeyword,
        b"boolean" => ast::Kind::BooleanKeyword,
        b"constructor" => ast::Kind::ConstructorKeyword,
        b"declare" => ast::Kind::DeclareKeyword,
        b"get" => ast::Kind::GetKeyword,
        b"immediate" => ast::Kind::ImmediateKeyword,
        b"infer" => ast::Kind::InferKeyword,
        b"intrinsic" => ast::Kind::IntrinsicKeyword,
        b"is" => ast::Kind::IsKeyword,
        b"keyof" => ast::Kind::KeyOfKeyword,
        b"module" => ast::Kind::ModuleKeyword,
        b"namespace" => ast::Kind::NamespaceKeyword,
        b"never" => ast::Kind::NeverKeyword,
        b"out" => ast::Kind::OutKeyword,
        b"readonly" => ast::Kind::ReadonlyKeyword,
        b"require" => ast::Kind::RequireKeyword,
        b"number" => ast::Kind::NumberKeyword,
        b"object" => ast::Kind::ObjectKeyword,
        b"satisfies" => ast::Kind::SatisfiesKeyword,
        b"set" => ast::Kind::SetKeyword,
        b"string" => ast::Kind::StringKeyword,
        b"symbol" => ast::Kind::SymbolKeyword,
        b"type" => ast::Kind::TypeKeyword,
        b"undefined" => ast::Kind::UndefinedKeyword,
        b"unique" => ast::Kind::UniqueKeyword,
        b"unknown" => ast::Kind::UnknownKeyword,
        b"using" => ast::Kind::UsingKeyword,
        b"from" => ast::Kind::FromKeyword,
        b"global" => ast::Kind::GlobalKeyword,
        b"bigint" => ast::Kind::BigIntKeyword,
        b"override" => ast::Kind::OverrideKeyword,
        b"of" => ast::Kind::OfKeyword,
        b"defer" => ast::Kind::DeferKeyword,
        _ => ast::Kind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_text_range_helper_slices_without_trivia_adjustment() {
        let text = "/** doc */\nconst x = 1;";
        let range = core::new_text_range(0, 10);

        assert_eq!(
            get_source_text_of_node_from_source_text(text, range),
            "/** doc */"
        );
    }
}
