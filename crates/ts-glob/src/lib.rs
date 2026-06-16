#![forbid(unsafe_code)]
use std::{error::Error, fmt};

// Copyright 2023 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

// A Glob is an LSP-compliant glob pattern, as defined by the spec:
// https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#documentFilter
//
// NOTE: this implementation is currently only intended for testing. In order
// to make it production ready, we'd need to:
//   - verify it against the VS Code implementation
//   - add more tests
//   - microbenchmark, likely avoiding the element interface
//   - resolve the question of what is meant by "character". If it's a UTF-16
//     code (as we suspect) it'll be a bit more work.
//
// Quoting from the spec:
// Glob patterns can have the following syntax:
//   - `*` to match one or more characters in a path segment
//   - `?` to match on one character in a path segment
//   - `**` to match any number of path segments, including none
//   - `{}` to group sub patterns into an OR expression. (e.g. `**/*.{ts,js}`
//     matches all TypeScript and JavaScript files)
//   - `[]` to declare a range of characters to match in a path segment
//     (e.g., `example.[0-9]` to match on `example.0`, `example.1`, ...)
//   - `[!...]` to negate a range of characters to match in a path segment
//     (e.g., `example.[!0-9]` to match on `example.a`, `example.b`, but
//     not `example.0`)
//
// Expanding on this:
//   - '/' matches one or more literal slashes.
//   - any other character matches itself literally.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Glob {
    elems: Vec<Element>, // pattern elements
}

// Parse builds a Glob for the given pattern, returning an error if the pattern
// is invalid.
pub fn parse(pattern: &str) -> Result<Glob, GlobError> {
    let (g, _, err) = parse_worker(pattern, false);
    err.map_or(Ok(g), Err)
}

fn parse_worker(pattern: &str, nested: bool) -> (Glob, &str, Option<GlobError>) {
    let mut g = Glob::default();
    let mut pattern = pattern;
    while !pattern.is_empty() {
        match pattern.as_bytes()[0] {
            b'/' => {
                pattern = &pattern[1..];
                g.elems.push(Element::Slash);
            }
            b'*' => {
                if pattern.len() > 1 && pattern.as_bytes()[1] == b'*' {
                    if (!g.elems.is_empty() && g.elems.last() != Some(&Element::Slash))
                        || (pattern.len() > 2 && pattern.as_bytes()[2] != b'/')
                    {
                        return (
                            Glob::default(),
                            "",
                            Some(GlobError::Message(
                                "** may only be adjacent to '/'".to_string(),
                            )),
                        );
                    }
                    pattern = &pattern[2..];
                    g.elems.push(Element::StarStar);
                    continue;
                }
                pattern = &pattern[1..];
                g.elems.push(Element::Star);
            }
            b'?' => {
                pattern = &pattern[1..];
                g.elems.push(Element::AnyChar);
            }
            b'{' => {
                let mut gs = Vec::new();
                while !pattern.is_empty() && pattern.as_bytes()[0] != b'}' {
                    pattern = &pattern[1..];
                    let (group_g, pat, err) = parse_worker(pattern, true);
                    if let Some(err) = err {
                        return (Glob::default(), "", Some(err));
                    }
                    if pat.is_empty() {
                        return (
                            Glob::default(),
                            "",
                            Some(GlobError::Message("unmatched '{'".to_string())),
                        );
                    }
                    pattern = pat;
                    gs.push(group_g);
                }
                if pattern.is_empty() {
                    return (
                        Glob::default(),
                        "",
                        Some(GlobError::Message("unmatched '{'".to_string())),
                    );
                }
                pattern = &pattern[1..];
                g.elems.push(Element::Group(gs));
            }
            b'}' | b',' => {
                if nested {
                    return (g, pattern, None);
                }
                pattern = g.parse_literal(pattern, nested);
            }
            b'[' => {
                pattern = &pattern[1..];
                if pattern.is_empty() {
                    return (Glob::default(), "", Some(GlobError::BadRange));
                }
                let mut negate = false;
                if pattern.as_bytes()[0] == b'!' {
                    pattern = &pattern[1..];
                    negate = true;
                }
                let (low, sz, err) = read_range_rune(pattern);
                if let Some(err) = err {
                    return (Glob::default(), "", Some(err));
                }
                pattern = &pattern[sz..];
                if pattern.is_empty() || pattern.as_bytes()[0] != b'-' {
                    return (Glob::default(), "", Some(GlobError::BadRange));
                }
                pattern = &pattern[1..];
                let (high, sz, err) = read_range_rune(pattern);
                if let Some(err) = err {
                    return (Glob::default(), "", Some(err));
                }
                pattern = &pattern[sz..];
                if pattern.is_empty() || pattern.as_bytes()[0] != b']' {
                    return (Glob::default(), "", Some(GlobError::BadRange));
                }
                pattern = &pattern[1..];
                g.elems.push(Element::CharRange { negate, low, high });
            }
            _ => {
                pattern = g.parse_literal(pattern, nested);
            }
        }
    }
    (g, "", None)
}

// helper for decoding a rune in range elements, e.g. [a-z]
fn read_range_rune(input: &str) -> (char, usize, Option<GlobError>) {
    let Some(r) = input.chars().next() else {
        return ('\0', 0, Some(GlobError::BadRange));
    };
    (r, r.len_utf8(), None)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GlobError {
    BadRange,
    InvalidUtf8,
    Message(String),
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobError::BadRange => write!(f, "'[' patterns must be of the form [x-y]"),
            GlobError::InvalidUtf8 => write!(f, "invalid UTF-8 encoding"),
            GlobError::Message(message) => f.write_str(message),
        }
    }
}

impl Error for GlobError {}

impl Glob {
    fn parse_literal<'a>(&mut self, pattern: &'a str, nested: bool) -> &'a str {
        let special_chars = if nested { "*?{[/}," } else { "*?{[/" };
        let end = pattern
            .find(|ch| special_chars.contains(ch))
            .unwrap_or(pattern.len());
        self.elems
            .push(Element::Literal(pattern[..end].to_string()));
        &pattern[end..]
    }

    // Match reports whether the input string matches the glob pattern.
    pub fn match_input(&self, input: &str) -> bool {
        matches(&self.elems, input)
    }
}

impl fmt::Display for Glob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for e in &self.elems {
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

// element holds a glob pattern element, as defined below.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Element {
    Slash,                                             // One or more '/' separators
    Literal(String),  // string literal, not containing /, *, ?, {}, or []
    Star,             // *
    AnyChar,          // ?
    StarStar,         // **
    Group(Vec<Glob>), // {foo, bar, ...} grouping
    CharRange { negate: bool, low: char, high: char }, // [a-z] character range
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Element::Slash => f.write_str("/"),
            Element::Literal(l) => f.write_str(l),
            Element::Star => f.write_str("*"),
            Element::AnyChar => f.write_str("?"),
            Element::StarStar => f.write_str("**"),
            Element::Group(g) => {
                let parts = g.iter().map(ToString::to_string).collect::<Vec<_>>();
                write!(f, "{{{}}}", parts.join(","))
            }
            Element::CharRange { low, high, .. } => write!(f, "[{low}-{high}]"),
        }
    }
}

fn matches(elems: &[Element], input: &str) -> bool {
    let mut elems = elems;
    let mut input = input;
    while !elems.is_empty() {
        let (elem, rest) = elems.split_first().expect("checked non-empty");
        elems = rest;
        match elem {
            Element::Slash => {
                if input.is_empty() || !input.starts_with('/') {
                    return false;
                }
                while input.starts_with('/') {
                    input = &input[1..];
                }
            }
            Element::StarStar => {
                // Special cases:
                //  - **/a matches "a"
                //  - **/ matches everything
                //
                // Note that if ** is followed by anything, it must be '/' (this is
                // enforced by Parse).
                if !elems.is_empty() {
                    elems = &elems[1..];
                }

                // A trailing ** matches anything.
                if elems.is_empty() {
                    return true;
                }

                // Backtracking: advance pattern segments until the remaining pattern
                // elements match.
                while !input.is_empty() {
                    if matches(elems, input) {
                        return true;
                    }
                    let (_, rest) = split(input);
                    input = rest;
                }
                return false;
            }
            Element::Literal(literal) => {
                if !input.starts_with(literal) {
                    return false;
                }
                input = &input[literal.len()..];
            }
            Element::Star => {
                let (seg_input, rest_input) = split(input);
                input = rest_input;

                let mut elem_end = elems.len();
                for (i, e) in elems.iter().enumerate() {
                    if *e == Element::Slash {
                        elem_end = i;
                        break;
                    }
                }
                let seg_elems = &elems[..elem_end];
                elems = &elems[elem_end..];

                // A trailing * matches the entire segment.
                if seg_elems.is_empty() {
                    continue;
                }

                // Backtracking: advance characters until remaining subpattern elements
                // match.
                let mut matched = false;
                for (i, _) in seg_input.char_indices() {
                    if matches(seg_elems, &seg_input[i..]) {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return false;
                }
            }
            Element::AnyChar => {
                if input.is_empty() || input.starts_with('/') {
                    return false;
                }
                let ch_len = input.chars().next().map(char::len_utf8).unwrap_or(1);
                input = &input[ch_len..];
            }
            Element::Group(group) => {
                // Append remaining pattern elements to each group member looking for a
                // match.
                for m in group {
                    let mut branch = m.elems.clone();
                    branch.extend_from_slice(elems);
                    if matches(&branch, input) {
                        return true;
                    }
                }
                return false;
            }
            Element::CharRange { low, high, .. } => {
                if input.is_empty() || input.starts_with('/') {
                    return false;
                }
                let Some(c) = input.chars().next() else {
                    return false;
                };
                if c < *low || c > *high {
                    return false;
                }
                input = &input[c.len_utf8()..];
            }
        }
    }

    input.is_empty()
}

// split returns the portion before and after the first slash
// (or sequence of consecutive slashes). If there is no slash
// it returns (input, nil).
fn split(input: &str) -> (&str, &str) {
    let Some(i) = input.find('/') else {
        return (input, "");
    };
    let first = &input[..i];
    for j in i..input.len() {
        if input.as_bytes()[j] != b'/' {
            return (first, &input[j..]);
        }
    }
    (first, "")
}
