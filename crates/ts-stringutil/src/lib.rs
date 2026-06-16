#![forbid(unsafe_code)]
use std::cmp::Ordering;

#[cfg(test)]
mod util_test;

pub fn equate_string_case_insensitive(a: &str, b: &str) -> bool {
    // !!!
    // return a == b || strings.ToUpper(a) == strings.ToUpper(b)
    a == b || a.to_lowercase() == b.to_lowercase()
}

pub fn equate_string_case_sensitive(a: &str, b: &str) -> bool {
    a == b
}

pub fn get_string_equality_comparer(ignore_case: bool) -> fn(&str, &str) -> bool {
    if ignore_case {
        equate_string_case_insensitive
    } else {
        equate_string_case_sensitive
    }
}

pub type Comparison = i32;

pub const COMPARISON_LESS_THAN: Comparison = -1;
pub const COMPARISON_EQUAL: Comparison = 0;
pub const COMPARISON_GREATER_THAN: Comparison = 1;

pub fn compare_strings_case_insensitive(mut a: &str, mut b: &str) -> Comparison {
    if a == b {
        return COMPARISON_EQUAL;
    }

    loop {
        let mut a_chars = a.chars();
        let mut b_chars = b.chars();
        let ca = a_chars.next();
        let cb = b_chars.next();

        match (ca, cb) {
            (None, None) => return COMPARISON_EQUAL,
            (None, Some(_)) => return COMPARISON_LESS_THAN,
            (Some(_), None) => return COMPARISON_GREATER_THAN,
            (Some(ca), Some(cb)) => {
                let lca = ca.to_lowercase().next().unwrap_or(ca);
                let lcb = cb.to_lowercase().next().unwrap_or(cb);
                if lca != lcb {
                    return if lca < lcb {
                        COMPARISON_LESS_THAN
                    } else {
                        COMPARISON_GREATER_THAN
                    };
                }
                a = &a[ca.len_utf8()..];
                b = &b[cb.len_utf8()..];
            }
        }
    }
}

pub fn compare_strings_case_sensitive(a: &str, b: &str) -> Comparison {
    match a.cmp(b) {
        Ordering::Less => COMPARISON_LESS_THAN,
        Ordering::Equal => COMPARISON_EQUAL,
        Ordering::Greater => COMPARISON_GREATER_THAN,
    }
}

pub fn get_string_comparer(ignore_case: bool) -> fn(&str, &str) -> Comparison {
    if ignore_case {
        compare_strings_case_insensitive
    } else {
        compare_strings_case_sensitive
    }
}

pub fn has_prefix(s: &str, prefix: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        return s.starts_with(prefix);
    }
    if prefix.len() > s.len() || !s.is_char_boundary(prefix.len()) {
        return false;
    }
    equate_string_case_insensitive(&s[..prefix.len()], prefix)
}

pub fn has_suffix(s: &str, suffix: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        return s.ends_with(suffix);
    }
    if suffix.len() > s.len() {
        return false;
    }
    let start = s.len() - suffix.len();
    if !s.is_char_boundary(start) {
        return false;
    }
    equate_string_case_insensitive(&s[start..], suffix)
}

pub fn has_prefix_and_suffix_without_overlap(
    s: &str,
    prefix: &str,
    suffix: &str,
    case_sensitive: bool,
) -> bool {
    if prefix.len() + suffix.len() > s.len() {
        return false;
    }

    has_prefix(s, prefix, case_sensitive) && has_suffix(s, suffix, case_sensitive)
}

pub fn compare_strings_case_insensitive_then_sensitive(a: &str, b: &str) -> Comparison {
    let cmp = compare_strings_case_insensitive(a, b);
    if cmp != COMPARISON_EQUAL {
        return cmp;
    }
    compare_strings_case_sensitive(a, b)
}

// CompareStringsCaseInsensitiveEslintCompatible performs a case-insensitive comparison
// using toLowerCase() instead of toUpperCase() for ESLint compatibility.
//
// `CompareStringsCaseInsensitive` transforms letters to uppercase for unicode reasons,
// while eslint's `sort-imports` rule transforms letters to lowercase. Which one you choose
// affects the relative order of letters and ASCII characters 91-96, of which `_` is a
// valid character in an identifier. So if we used `CompareStringsCaseInsensitive` for
// import sorting, TypeScript and eslint would disagree about the correct case-insensitive
// sort order for `__String` and `Foo`. Since eslint's whole job is to create consistency
// by enforcing nitpicky details like this, it makes way more sense for us to just adopt
// their convention so users can have auto-imports without making eslint angry.
pub fn compare_strings_case_insensitive_eslint_compatible(a: &str, b: &str) -> Comparison {
    if a == b {
        return COMPARISON_EQUAL;
    }
    compare_strings_case_sensitive(&a.to_lowercase(), &b.to_lowercase())
}

pub fn is_white_space_like(ch: char) -> bool {
    is_white_space_single_line(ch) || is_line_break(ch)
}

pub fn is_white_space_single_line(ch: char) -> bool {
    // Note: nextLine is in the Zs space, and should be considered to be a whitespace.
    // It is explicitly not a line-break as it isn't in the exact set specified by EcmaScript.
    matches!(
        ch,
        ' ' | '\t'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            | '\u{2001}'
            | '\u{2002}'
            | '\u{2003}'
            | '\u{2004}'
            | '\u{2005}'
            | '\u{2006}'
            | '\u{2007}'
            | '\u{2008}'
            | '\u{2009}'
            | '\u{200A}'
            | '\u{200B}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

pub fn is_line_break(ch: char) -> bool {
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

pub fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

pub fn is_octal_digit(ch: char) -> bool {
    ('0'..='7').contains(&ch)
}

pub fn is_hex_digit(ch: char) -> bool {
    ch.is_ascii_hexdigit()
}

pub fn is_ascii_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

pub fn strip_quotes(name: &str) -> &str {
    if name.len() >= 2 {
        let bytes = name.as_bytes();
        let first = bytes[0];
        let last = bytes[name.len() - 1];
        if matches!(first, b'\'' | b'"' | b'`') && first == last {
            return &name[1..name.len() - 1];
        }
    }
    name
}

pub fn unquote_string(str_: &str) -> String {
    let inner = strip_quotes(str_);
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                result.push(next);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn truncate_by_runes(str: &str, max_length: i32) -> &str {
    if max_length > 0 && str.len() < max_length as usize {
        return str;
    }
    if max_length <= 0 {
        return "";
    }
    let mut rune_count = 0;
    for (i, _) in str.char_indices() {
        rune_count += 1;
        if rune_count > max_length {
            return &str[..i];
        }
    }
    str
}

pub fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::with_capacity(text.matches('\n').count() + 1);
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut pos = 0;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\r' => {
                lines.push(&text[start..pos]);
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'\n' {
                    pos += 2;
                } else {
                    pos += 1;
                }
                start = pos;
            }
            b'\n' => {
                lines.push(&text[start..pos]);
                pos += 1;
                start = pos;
            }
            _ => {
                pos += 1;
            }
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

pub fn guess_indentation(lines: &[&str]) -> usize {
    const MAX_SMI_X86: usize = 0x3fff_ffff;
    let mut indentation = MAX_SMI_X86;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let mut i = 0;
        while i < line.len() && i < indentation {
            let ch = line[i..].chars().next().unwrap();
            if !is_white_space_like(ch) {
                break;
            }
            i += ch.len_utf8();
        }
        if i < indentation {
            indentation = i;
        }
        if indentation == 0 {
            return 0;
        }
    }
    if indentation == MAX_SMI_X86 {
        0
    } else {
        indentation
    }
}

// https://tc39.es/ecma262/multipage/global-object.html#sec-encodeuri-uri
pub fn encode_uri(s: &str) -> String {
    let mut builder = String::new();
    for b in s.as_bytes() {
        if !should_escape_for_encode_uri(*b) {
            builder.push(*b as char);
            continue;
        }

        builder.push('%');
        builder.push(UPPERHEX[(b >> 4) as usize] as char);
        builder.push(UPPERHEX[(b & 0x0f) as usize] as char);
    }
    builder
}

const UPPERHEX: &[u8; 16] = b"0123456789ABCDEF";

fn should_escape_for_encode_uri(b: u8) -> bool {
    if b.is_ascii_alphanumeric() {
        return false;
    }

    !matches!(
        b,
        b';' | b'/'
            | b'?'
            | b':'
            | b'@'
            | b'&'
            | b'='
            | b'+'
            | b'$'
            | b','
            | b'#'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')'
    )
}

fn get_byte_order_mark_length(text: &str) -> usize {
    let bytes = text.as_bytes();
    if !bytes.is_empty() {
        let ch0 = bytes[0];
        if ch0 == 0xfe {
            if bytes.len() >= 2 && bytes[1] == 0xff {
                return 2; // utf16be
            }
            return 0;
        }
        if ch0 == 0xff {
            if bytes.len() >= 2 && bytes[1] == 0xfe {
                return 2; // utf16le
            }
            return 0;
        }
        if ch0 == 0xef {
            if bytes.len() >= 3 && bytes[1] == 0xbb && bytes[2] == 0xbf {
                return 3; // utf8
            }
            return 0;
        }
    }
    0
}

pub fn remove_byte_order_mark(text: &str) -> &str {
    let length = get_byte_order_mark_length(text);
    if length > 0 { &text[length..] } else { text }
}

pub fn add_utf8_byte_order_mark(text: &str) -> String {
    if get_byte_order_mark_length(text) == 0 {
        format!("\u{FEFF}{text}")
    } else {
        text.to_owned()
    }
}
