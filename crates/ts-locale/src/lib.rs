#![forbid(unsafe_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Locale(String);

pub const DEFAULT: Locale = Locale(String::new());

impl Locale {
    pub fn und() -> Self {
        Self::default()
    }

    pub fn is_und(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn und() -> Locale {
    Locale::und()
}

impl From<&str> for Locale {
    fn from(value: &str) -> Self {
        Locale(value.to_owned())
    }
}

impl From<String> for Locale {
    fn from(value: String) -> Self {
        Locale(value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Context {
    locale: Option<Locale>,
}

pub fn with_locale(mut ctx: Context, locale: Locale) -> Context {
    ctx.locale = Some(locale);
    ctx
}

pub fn from_context(ctx: &Context) -> Locale {
    ctx.locale.clone().unwrap_or_default()
}

pub fn parse(locale_str: &str) -> (Locale, bool) {
    // Parse gracefully fails.
    match parse_language_tag(locale_str) {
        Some(tag) => (Locale(tag), true),
        None => (Locale::default(), false),
    }
}

fn parse_language_tag(locale_str: &str) -> Option<String> {
    if locale_str.is_empty() || locale_str.len() > 255 || locale_str.contains('_') {
        return None;
    }

    let lower = locale_str.to_ascii_lowercase();
    if is_grandfathered_tag(&lower) {
        return Some(lower);
    }

    let parts: Vec<&str> = locale_str.split('-').collect();
    if parts.iter().any(|part| part.is_empty() || !is_alnum(part)) {
        return None;
    }

    if parts
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("x"))
    {
        return parse_private_use(&parts).then(|| canonicalize_private_use(&parts));
    }

    let mut index = parse_language(&parts)?;
    let mut canonical = Vec::with_capacity(parts.len());
    canonical.push(parts[0].to_ascii_lowercase());

    while index < parts.len() && is_extlang(parts[index]) {
        canonical.push(parts[index].to_ascii_lowercase());
        index += 1;
    }

    if index < parts.len() && is_script(parts[index]) {
        canonical.push(canonicalize_script(parts[index]));
        index += 1;
    }

    if index < parts.len() && is_region(parts[index]) {
        canonical.push(parts[index].to_ascii_uppercase());
        index += 1;
    }

    while index < parts.len() && is_variant(parts[index]) {
        canonical.push(parts[index].to_ascii_lowercase());
        index += 1;
    }

    while index < parts.len() {
        let singleton = parts[index];
        if singleton.eq_ignore_ascii_case("x") {
            if !parse_private_use(&parts[index..]) {
                return None;
            }
            canonical.extend(parts[index..].iter().map(|part| part.to_ascii_lowercase()));
            return Some(canonical.join("-"));
        }
        if !is_extension_singleton(singleton) {
            return None;
        }
        canonical.push(singleton.to_ascii_lowercase());
        index += 1;
        let extension_start = index;
        while index < parts.len() && is_extension_subtag(parts[index]) {
            canonical.push(parts[index].to_ascii_lowercase());
            index += 1;
        }
        if index == extension_start {
            return None;
        }
    }

    Some(canonical.join("-"))
}

fn parse_language(parts: &[&str]) -> Option<usize> {
    let language = parts.first()?;
    let len = language.len();
    if !is_alpha(language) {
        return None;
    }
    match len {
        2 | 3 | 4 => Some(1),
        _ => None,
    }
}

fn parse_private_use(parts: &[&str]) -> bool {
    parts.len() >= 2
        && parts[0].eq_ignore_ascii_case("x")
        && parts[1..]
            .iter()
            .all(|part| (1..=8).contains(&part.len()) && is_alnum(part))
}

fn canonicalize_private_use(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

fn canonicalize_script(script: &str) -> String {
    let mut chars = script.chars();
    let first = chars.next().unwrap().to_ascii_uppercase();
    let rest = chars.as_str().to_ascii_lowercase();
    format!("{first}{rest}")
}

fn is_alpha(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_alnum(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn is_extlang(text: &str) -> bool {
    text.len() == 3 && is_alpha(text)
}

fn is_script(text: &str) -> bool {
    text.len() == 4 && is_alpha(text)
}

fn is_region(text: &str) -> bool {
    (text.len() == 2 && is_alpha(text))
        || (text.len() == 3 && text.bytes().all(|b| b.is_ascii_digit()))
}

fn is_variant(text: &str) -> bool {
    (5..=8).contains(&text.len()) && is_alnum(text)
        || text.len() == 4
            && text.as_bytes()[0].is_ascii_digit()
            && text.as_bytes()[1..].iter().all(u8::is_ascii_alphanumeric)
}

fn is_extension_singleton(text: &str) -> bool {
    text.len() == 1 && is_alnum(text) && !text.eq_ignore_ascii_case("x")
}

fn is_extension_subtag(text: &str) -> bool {
    (2..=8).contains(&text.len()) && is_alnum(text)
}

fn is_grandfathered_tag(tag: &str) -> bool {
    matches!(
        tag,
        "art-lojban"
            | "cel-gaulish"
            | "en-gb-oed"
            | "i-ami"
            | "i-bnn"
            | "i-default"
            | "i-enochian"
            | "i-hak"
            | "i-klingon"
            | "i-lux"
            | "i-mingo"
            | "i-navajo"
            | "i-pwn"
            | "i-tao"
            | "i-tay"
            | "i-tsu"
            | "no-bok"
            | "no-nyn"
            | "sgn-be-fr"
            | "sgn-be-nl"
            | "sgn-ch-de"
            | "zh-guoyu"
            | "zh-hakka"
            | "zh-min"
            | "zh-min-nan"
            | "zh-xiang"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_and_canonicalizes_language_tags() {
        let cases = [
            ("en", "en"),
            ("de-de", "de-DE"),
            ("zh-hant-tw", "zh-Hant-TW"),
            ("sl-rozaj-biske-1994", "sl-rozaj-biske-1994"),
            ("en-u-ca-gregory", "en-u-ca-gregory"),
            ("x-private-tag", "x-private-tag"),
        ];

        for (input, expected) in cases {
            let (locale, ok) = parse(input);
            assert!(ok, "{input}");
            assert_eq!(locale.as_str(), expected);
        }
    }

    #[test]
    fn parse_rejects_invalid_language_tags() {
        for input in [
            "",
            "en_",
            "en--US",
            "e",
            "whoops",
            "abcdefghi",
            "en-@",
            "en-u",
            "x",
        ] {
            let (locale, ok) = parse(input);
            assert!(!ok, "{input}");
            assert!(locale.is_und());
        }
    }

    #[test]
    fn locale_context_round_trips() {
        let (locale, ok) = parse("ja-jp");
        assert!(ok);
        let ctx = with_locale(Context::default(), locale.clone());
        assert_eq!(from_context(&ctx), locale);
        assert_eq!(from_context(&Context::default()), Locale::default());
    }
}
