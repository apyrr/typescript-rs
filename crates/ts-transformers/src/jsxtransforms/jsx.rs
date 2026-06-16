use ts_ast as ast;
use ts_core::{JsxEmit, ScriptTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsxAction {
    Keep,
    VisitChildren,
    SkipSourceFile,
    TransformSourceFile,
    TransformJsxElement,
    TransformJsxSelfClosingElement,
    TransformJsxFragment,
    TransformJsxText,
    TransformJsxExpression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsxFactoryMode {
    CreateElement,
    AutomaticRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsxImplicitImportMode {
    None,
    EsImportDeclaration,
    CommonJsRequireBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsxAttributeObjectMode {
    ObjectLiteralWithSpreads,
    AssignHelperExpression,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JsxFacts {
    pub subtree_contains_jsx: bool,
    pub is_declaration_file: bool,
    pub has_implicit_import_specifier: bool,
    pub has_key_after_props_spread: bool,
    pub is_external_module: bool,
    pub is_external_or_common_js_module: bool,
    pub has_multiple_semantic_children: bool,
    pub has_spread_child: bool,
}

pub fn jsx_action_for_kind(kind: ast::Kind, facts: JsxFacts) -> JsxAction {
    if !facts.subtree_contains_jsx {
        return JsxAction::Keep;
    }

    match kind {
        ast::Kind::SourceFile if facts.is_declaration_file => JsxAction::SkipSourceFile,
        ast::Kind::SourceFile => JsxAction::TransformSourceFile,
        ast::Kind::JsxElement => JsxAction::TransformJsxElement,
        ast::Kind::JsxSelfClosingElement => JsxAction::TransformJsxSelfClosingElement,
        ast::Kind::JsxFragment => JsxAction::TransformJsxFragment,
        ast::Kind::JsxText => JsxAction::TransformJsxText,
        ast::Kind::JsxExpression => JsxAction::TransformJsxExpression,
        _ => JsxAction::VisitChildren,
    }
}

pub fn jsx_factory_callee_primitive(jsx: JsxEmit, is_static_children: bool) -> &'static str {
    if jsx == JsxEmit::ReactJSXDev {
        "jsxDEV"
    } else if is_static_children {
        "jsxs"
    } else {
        "jsx"
    }
}

pub fn should_use_create_element(facts: JsxFacts) -> bool {
    !facts.has_implicit_import_specifier || facts.has_key_after_props_spread
}

pub fn jsx_factory_mode(facts: JsxFacts) -> JsxFactoryMode {
    if should_use_create_element(facts) {
        JsxFactoryMode::CreateElement
    } else {
        JsxFactoryMode::AutomaticRuntime
    }
}

pub fn implicit_import_mode(facts: JsxFacts, used_runtime_imports: bool) -> JsxImplicitImportMode {
    if !used_runtime_imports {
        return JsxImplicitImportMode::None;
    }
    if facts.is_external_module {
        JsxImplicitImportMode::EsImportDeclaration
    } else if facts.is_external_or_common_js_module {
        JsxImplicitImportMode::CommonJsRequireBinding
    } else {
        JsxImplicitImportMode::None
    }
}

pub fn jsx_attribute_object_mode(target: ScriptTarget) -> JsxAttributeObjectMode {
    if target >= ScriptTarget::ES2018 {
        JsxAttributeObjectMode::ObjectLiteralWithSpreads
    } else {
        JsxAttributeObjectMode::AssignHelperExpression
    }
}

pub fn is_static_children(facts: JsxFacts) -> bool {
    facts.has_multiple_semantic_children || facts.has_spread_child
}

pub fn jsx_attribute_initializer_default_true(is_missing: bool, is_empty_expression: bool) -> bool {
    is_missing || is_empty_expression
}

pub fn jsx_text_to_string_literal(text: &str) -> Option<String> {
    let fixed = fixup_whitespace_and_decode_entities(text);
    (!fixed.is_empty()).then_some(fixed)
}

pub fn fixup_whitespace_and_decode_entities(text: &str) -> String {
    let mut acc = String::new();
    let mut initial = true;
    // First non-whitespace character on this line.
    let mut first_non_whitespace = Some(0);
    // End byte position of the last non-whitespace character on this line.
    let mut last_non_whitespace_end = None;
    // These initial values are special because the first line is:
    // firstNonWhitespace = 0 to indicate that we want leading whitespace,
    // but lastNonWhitespaceEnd = -1 as a special flag to indicate that we *don't* include the line if it's all whitespace.
    for (index, c) in text.char_indices() {
        if is_line_break(c) {
            // If we've seen any non-whitespace characters on this line, add the 'trim' of the line.
            // (lastNonWhitespaceEnd === -1 is a special flag to detect whether the first line is all whitespace.)
            if let (Some(first), Some(last)) = (first_non_whitespace, last_non_whitespace_end) {
                add_line_of_jsx_text(&mut acc, &text[first..last], initial);
                initial = false;
            }

            // Reset firstNonWhitespace for the next line.
            // Don't bother to reset lastNonWhitespaceEnd because we ignore it if firstNonWhitespace = -1.
            first_non_whitespace = None;
        } else if !is_white_space_single_line(c) {
            last_non_whitespace_end = Some(index + c.len_utf8());
            if first_non_whitespace.is_none() {
                first_non_whitespace = Some(index);
            }
        }
    }

    if let Some(first) = first_non_whitespace {
        // Last line had a non-whitespace character. Emit the 'trimLeft', meaning keep trailing whitespace.
        add_line_of_jsx_text(&mut acc, &text[first..], initial);
    }

    acc
}

fn add_line_of_jsx_text(acc: &mut String, trimmed_line: &str, is_initial: bool) {
    // We do not escape the string here as that is handled by the printer
    // when it emits the literal. We do, however, need to decode JSX entities.
    let decoded = decode_entities(trimmed_line);
    if !is_initial {
        acc.push(' ');
    }
    acc.push_str(&decoded);
}

/**
 * Replace entities like "&nbsp;", "&#123;", and "&#xDEADBEEF;" with the characters they encode.
 * See https://en.wikipedia.org/wiki/List_of_XML_and_HTML_character_entity_references
 */
pub fn decode_entities(text: &str) -> String {
    let Some(first_ampersand) = text.find('&') else {
        return text.to_owned();
    };

    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..first_ampersand]);
    let mut rest = &text[first_ampersand..];
    while let Some(stripped) = rest.strip_prefix('&') {
        let Some(semi) = stripped.find(';') else {
            result.push('&');
            result.push_str(stripped);
            return result;
        };
        let entity = &stripped[..semi];
        if let Some(decoded) = decode_entity(entity) {
            result.push(decoded);
        } else {
            result.push('&');
            result.push_str(entity);
            result.push(';');
        }
        rest = &stripped[semi + 1..];
        if let Some(next_ampersand) = rest.find('&') {
            result.push_str(&rest[..next_ampersand]);
            rest = &rest[next_ampersand..];
        } else {
            result.push_str(rest);
            break;
        }
    }
    result
}

pub fn decode_entity(entity: &str) -> Option<char> {
    if entity.is_empty() {
        return None;
    }

    if let Some(mut number) = entity.strip_prefix('#') {
        if number.is_empty() {
            return None;
        }

        let mut radix = 10;
        if let Some(hex) = number
            .strip_prefix('x')
            .or_else(|| number.strip_prefix('X'))
        {
            radix = 16;
            number = hex;
        }

        if number.is_empty() {
            return None;
        }

        if !number.chars().all(|c| {
            if radix == 16 {
                is_hex_digit(c)
            } else {
                is_digit(c)
            }
        }) {
            return None;
        }

        return u32::from_str_radix(number, radix)
            .ok()
            .and_then(char::from_u32);
    }

    match entity {
        "quot" => char::from_u32(0x0022),
        "amp" => char::from_u32(0x0026),
        "apos" => char::from_u32(0x0027),
        "lt" => char::from_u32(0x003C),
        "gt" => char::from_u32(0x003E),
        "nbsp" => char::from_u32(0x00A0),
        "iexcl" => char::from_u32(0x00A1),
        "cent" => char::from_u32(0x00A2),
        "pound" => char::from_u32(0x00A3),
        "curren" => char::from_u32(0x00A4),
        "yen" => char::from_u32(0x00A5),
        "brvbar" => char::from_u32(0x00A6),
        "sect" => char::from_u32(0x00A7),
        "uml" => char::from_u32(0x00A8),
        "copy" => char::from_u32(0x00A9),
        "ordf" => char::from_u32(0x00AA),
        "laquo" => char::from_u32(0x00AB),
        "not" => char::from_u32(0x00AC),
        "shy" => char::from_u32(0x00AD),
        "reg" => char::from_u32(0x00AE),
        "macr" => char::from_u32(0x00AF),
        "deg" => char::from_u32(0x00B0),
        "plusmn" => char::from_u32(0x00B1),
        "sup2" => char::from_u32(0x00B2),
        "sup3" => char::from_u32(0x00B3),
        "acute" => char::from_u32(0x00B4),
        "micro" => char::from_u32(0x00B5),
        "para" => char::from_u32(0x00B6),
        "middot" => char::from_u32(0x00B7),
        "cedil" => char::from_u32(0x00B8),
        "sup1" => char::from_u32(0x00B9),
        "ordm" => char::from_u32(0x00BA),
        "raquo" => char::from_u32(0x00BB),
        "frac14" => char::from_u32(0x00BC),
        "frac12" => char::from_u32(0x00BD),
        "frac34" => char::from_u32(0x00BE),
        "iquest" => char::from_u32(0x00BF),
        "Agrave" => char::from_u32(0x00C0),
        "Aacute" => char::from_u32(0x00C1),
        "Acirc" => char::from_u32(0x00C2),
        "Atilde" => char::from_u32(0x00C3),
        "Auml" => char::from_u32(0x00C4),
        "Aring" => char::from_u32(0x00C5),
        "AElig" => char::from_u32(0x00C6),
        "Ccedil" => char::from_u32(0x00C7),
        "Egrave" => char::from_u32(0x00C8),
        "Eacute" => char::from_u32(0x00C9),
        "Ecirc" => char::from_u32(0x00CA),
        "Euml" => char::from_u32(0x00CB),
        "Igrave" => char::from_u32(0x00CC),
        "Iacute" => char::from_u32(0x00CD),
        "Icirc" => char::from_u32(0x00CE),
        "Iuml" => char::from_u32(0x00CF),
        "ETH" => char::from_u32(0x00D0),
        "Ntilde" => char::from_u32(0x00D1),
        "Ograve" => char::from_u32(0x00D2),
        "Oacute" => char::from_u32(0x00D3),
        "Ocirc" => char::from_u32(0x00D4),
        "Otilde" => char::from_u32(0x00D5),
        "Ouml" => char::from_u32(0x00D6),
        "times" => char::from_u32(0x00D7),
        "Oslash" => char::from_u32(0x00D8),
        "Ugrave" => char::from_u32(0x00D9),
        "Uacute" => char::from_u32(0x00DA),
        "Ucirc" => char::from_u32(0x00DB),
        "Uuml" => char::from_u32(0x00DC),
        "Yacute" => char::from_u32(0x00DD),
        "THORN" => char::from_u32(0x00DE),
        "szlig" => char::from_u32(0x00DF),
        "agrave" => char::from_u32(0x00E0),
        "aacute" => char::from_u32(0x00E1),
        "acirc" => char::from_u32(0x00E2),
        "atilde" => char::from_u32(0x00E3),
        "auml" => char::from_u32(0x00E4),
        "aring" => char::from_u32(0x00E5),
        "aelig" => char::from_u32(0x00E6),
        "ccedil" => char::from_u32(0x00E7),
        "egrave" => char::from_u32(0x00E8),
        "eacute" => char::from_u32(0x00E9),
        "ecirc" => char::from_u32(0x00EA),
        "euml" => char::from_u32(0x00EB),
        "igrave" => char::from_u32(0x00EC),
        "iacute" => char::from_u32(0x00ED),
        "icirc" => char::from_u32(0x00EE),
        "iuml" => char::from_u32(0x00EF),
        "eth" => char::from_u32(0x00F0),
        "ntilde" => char::from_u32(0x00F1),
        "ograve" => char::from_u32(0x00F2),
        "oacute" => char::from_u32(0x00F3),
        "ocirc" => char::from_u32(0x00F4),
        "otilde" => char::from_u32(0x00F5),
        "ouml" => char::from_u32(0x00F6),
        "divide" => char::from_u32(0x00F7),
        "oslash" => char::from_u32(0x00F8),
        "ugrave" => char::from_u32(0x00F9),
        "uacute" => char::from_u32(0x00FA),
        "ucirc" => char::from_u32(0x00FB),
        "uuml" => char::from_u32(0x00FC),
        "yacute" => char::from_u32(0x00FD),
        "thorn" => char::from_u32(0x00FE),
        "yuml" => char::from_u32(0x00FF),
        "OElig" => char::from_u32(0x0152),
        "oelig" => char::from_u32(0x0153),
        "Scaron" => char::from_u32(0x0160),
        "scaron" => char::from_u32(0x0161),
        "Yuml" => char::from_u32(0x0178),
        "fnof" => char::from_u32(0x0192),
        "circ" => char::from_u32(0x02C6),
        "tilde" => char::from_u32(0x02DC),
        "Alpha" => char::from_u32(0x0391),
        "Beta" => char::from_u32(0x0392),
        "Gamma" => char::from_u32(0x0393),
        "Delta" => char::from_u32(0x0394),
        "Epsilon" => char::from_u32(0x0395),
        "Zeta" => char::from_u32(0x0396),
        "Eta" => char::from_u32(0x0397),
        "Theta" => char::from_u32(0x0398),
        "Iota" => char::from_u32(0x0399),
        "Kappa" => char::from_u32(0x039A),
        "Lambda" => char::from_u32(0x039B),
        "Mu" => char::from_u32(0x039C),
        "Nu" => char::from_u32(0x039D),
        "Xi" => char::from_u32(0x039E),
        "Omicron" => char::from_u32(0x039F),
        "Pi" => char::from_u32(0x03A0),
        "Rho" => char::from_u32(0x03A1),
        "Sigma" => char::from_u32(0x03A3),
        "Tau" => char::from_u32(0x03A4),
        "Upsilon" => char::from_u32(0x03A5),
        "Phi" => char::from_u32(0x03A6),
        "Chi" => char::from_u32(0x03A7),
        "Psi" => char::from_u32(0x03A8),
        "Omega" => char::from_u32(0x03A9),
        "alpha" => char::from_u32(0x03B1),
        "beta" => char::from_u32(0x03B2),
        "gamma" => char::from_u32(0x03B3),
        "delta" => char::from_u32(0x03B4),
        "epsilon" => char::from_u32(0x03B5),
        "zeta" => char::from_u32(0x03B6),
        "eta" => char::from_u32(0x03B7),
        "theta" => char::from_u32(0x03B8),
        "iota" => char::from_u32(0x03B9),
        "kappa" => char::from_u32(0x03BA),
        "lambda" => char::from_u32(0x03BB),
        "mu" => char::from_u32(0x03BC),
        "nu" => char::from_u32(0x03BD),
        "xi" => char::from_u32(0x03BE),
        "omicron" => char::from_u32(0x03BF),
        "pi" => char::from_u32(0x03C0),
        "rho" => char::from_u32(0x03C1),
        "sigmaf" => char::from_u32(0x03C2),
        "sigma" => char::from_u32(0x03C3),
        "tau" => char::from_u32(0x03C4),
        "upsilon" => char::from_u32(0x03C5),
        "phi" => char::from_u32(0x03C6),
        "chi" => char::from_u32(0x03C7),
        "psi" => char::from_u32(0x03C8),
        "omega" => char::from_u32(0x03C9),
        "thetasym" => char::from_u32(0x03D1),
        "upsih" => char::from_u32(0x03D2),
        "piv" => char::from_u32(0x03D6),
        "ensp" => char::from_u32(0x2002),
        "emsp" => char::from_u32(0x2003),
        "thinsp" => char::from_u32(0x2009),
        "zwnj" => char::from_u32(0x200C),
        "zwj" => char::from_u32(0x200D),
        "lrm" => char::from_u32(0x200E),
        "rlm" => char::from_u32(0x200F),
        "ndash" => char::from_u32(0x2013),
        "mdash" => char::from_u32(0x2014),
        "lsquo" => char::from_u32(0x2018),
        "rsquo" => char::from_u32(0x2019),
        "sbquo" => char::from_u32(0x201A),
        "ldquo" => char::from_u32(0x201C),
        "rdquo" => char::from_u32(0x201D),
        "bdquo" => char::from_u32(0x201E),
        "dagger" => char::from_u32(0x2020),
        "Dagger" => char::from_u32(0x2021),
        "bull" => char::from_u32(0x2022),
        "hellip" => char::from_u32(0x2026),
        "permil" => char::from_u32(0x2030),
        "prime" => char::from_u32(0x2032),
        "Prime" => char::from_u32(0x2033),
        "lsaquo" => char::from_u32(0x2039),
        "rsaquo" => char::from_u32(0x203A),
        "oline" => char::from_u32(0x203E),
        "frasl" => char::from_u32(0x2044),
        "euro" => char::from_u32(0x20AC),
        "image" => char::from_u32(0x2111),
        "weierp" => char::from_u32(0x2118),
        "real" => char::from_u32(0x211C),
        "trade" => char::from_u32(0x2122),
        "alefsym" => char::from_u32(0x2135),
        "larr" => char::from_u32(0x2190),
        "uarr" => char::from_u32(0x2191),
        "rarr" => char::from_u32(0x2192),
        "darr" => char::from_u32(0x2193),
        "harr" => char::from_u32(0x2194),
        "crarr" => char::from_u32(0x21B5),
        "lArr" => char::from_u32(0x21D0),
        "uArr" => char::from_u32(0x21D1),
        "rArr" => char::from_u32(0x21D2),
        "dArr" => char::from_u32(0x21D3),
        "hArr" => char::from_u32(0x21D4),
        "forall" => char::from_u32(0x2200),
        "part" => char::from_u32(0x2202),
        "exist" => char::from_u32(0x2203),
        "empty" => char::from_u32(0x2205),
        "nabla" => char::from_u32(0x2207),
        "isin" => char::from_u32(0x2208),
        "notin" => char::from_u32(0x2209),
        "ni" => char::from_u32(0x220B),
        "prod" => char::from_u32(0x220F),
        "sum" => char::from_u32(0x2211),
        "minus" => char::from_u32(0x2212),
        "lowast" => char::from_u32(0x2217),
        "radic" => char::from_u32(0x221A),
        "prop" => char::from_u32(0x221D),
        "infin" => char::from_u32(0x221E),
        "ang" => char::from_u32(0x2220),
        "and" => char::from_u32(0x2227),
        "or" => char::from_u32(0x2228),
        "cap" => char::from_u32(0x2229),
        "cup" => char::from_u32(0x222A),
        "int" => char::from_u32(0x222B),
        "there4" => char::from_u32(0x2234),
        "sim" => char::from_u32(0x223C),
        "cong" => char::from_u32(0x2245),
        "asymp" => char::from_u32(0x2248),
        "ne" => char::from_u32(0x2260),
        "equiv" => char::from_u32(0x2261),
        "le" => char::from_u32(0x2264),
        "ge" => char::from_u32(0x2265),
        "sub" => char::from_u32(0x2282),
        "sup" => char::from_u32(0x2283),
        "nsub" => char::from_u32(0x2284),
        "sube" => char::from_u32(0x2286),
        "supe" => char::from_u32(0x2287),
        "oplus" => char::from_u32(0x2295),
        "otimes" => char::from_u32(0x2297),
        "perp" => char::from_u32(0x22A5),
        "sdot" => char::from_u32(0x22C5),
        "lceil" => char::from_u32(0x2308),
        "rceil" => char::from_u32(0x2309),
        "lfloor" => char::from_u32(0x230A),
        "rfloor" => char::from_u32(0x230B),
        "lang" => char::from_u32(0x2329),
        "rang" => char::from_u32(0x232A),
        "loz" => char::from_u32(0x25CA),
        "spades" => char::from_u32(0x2660),
        "clubs" => char::from_u32(0x2663),
        "hearts" => char::from_u32(0x2665),
        "diams" => char::from_u32(0x2666),
        _ => None,
    }
}

fn is_white_space_single_line(ch: char) -> bool {
    // Note: nextLine is in the Zs space, and should be considered to be a whitespace.
    // It is explicitly not a line-break as it isn't in the exact set specified by EcmaScript.
    matches!(
        ch,
        ' ' | '\t' | '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200B}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

fn is_line_break(ch: char) -> bool {
    // ES5 7.3:
    // The ECMAScript line terminator characters are listed in Table 3.
    //     Table 3: Line Terminator Characters
    //     Code Unit Value     Name                    Formal Name
    //     \u000A              Line Feed               <LF>
    //     \u000D              Carriage Return         <CR>
    //     \u2028              Line separator          <LS>
    //     \u2029              Paragraph separator     <PS>
    // Only the characters in Table 3 are treated as line terminators. Other new line or line
    // breaking characters are treated as white space but not as line terminators.
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

fn is_hex_digit(ch: char) -> bool {
    ch.is_ascii_hexdigit()
}
