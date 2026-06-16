use ts_locale as locale;

use crate::*;

#[test]
fn test_localize() {
    struct Test {
        name: &'static str,
        message: &'static Message,
        locale: locale::Locale,
        args: Vec<Any>,
        expected: &'static str,
    }

    let tests = vec![
        Test {
            name: "english default",
            message: &Identifier_expected,
            locale: locale::Locale::from("en"),
            args: Vec::new(),
            expected: "Identifier expected.",
        },
        Test {
            name: "undefined locale uses english",
            message: &Identifier_expected,
            locale: locale::Locale::und(),
            args: Vec::new(),
            expected: "Identifier expected.",
        },
        Test {
            name: "with single argument",
            message: &X_0_expected,
            locale: locale::Locale::from("en"),
            args: vec![Box::new(")")],
            expected: "')' expected.",
        },
        Test {
            name: "with multiple arguments",
            message: &The_parser_expected_to_find_a_1_to_match_the_0_token_here,
            locale: locale::Locale::from("en"),
            args: vec![Box::new("{"), Box::new("}")],
            expected: "The parser expected to find a '}' to match the '{' token here.",
        },
        Test {
            name: "fallback to english for unknown locale",
            message: &Identifier_expected,
            locale: locale::Locale::from("af-ZA"),
            args: Vec::new(),
            expected: "Identifier expected.",
        },
        Test {
            name: "german",
            message: &Identifier_expected,
            locale: locale::Locale::from("de-DE"),
            args: Vec::new(),
            expected: "Es wurde ein Bezeichner erwartet.",
        },
        Test {
            name: "french",
            message: &Identifier_expected,
            locale: locale::Locale::from("fr-FR"),
            args: Vec::new(),
            expected: "Identificateur attendu.",
        },
        Test {
            name: "spanish",
            message: &Identifier_expected,
            locale: locale::Locale::from("es-ES"),
            args: Vec::new(),
            expected: "Se esperaba un identificador.",
        },
        Test {
            name: "japanese",
            message: &Identifier_expected,
            locale: locale::Locale::from("ja-JP"),
            args: Vec::new(),
            expected: "識別子が必要です。",
        },
        Test {
            name: "chinese simplified",
            message: &Identifier_expected,
            locale: locale::Locale::from("zh-CN"),
            args: Vec::new(),
            expected: "应为标识符。",
        },
        Test {
            name: "korean",
            message: &Identifier_expected,
            locale: locale::Locale::from("ko-KR"),
            args: Vec::new(),
            expected: "식별자가 필요합니다.",
        },
        Test {
            name: "russian",
            message: &Identifier_expected,
            locale: locale::Locale::from("ru-RU"),
            args: Vec::new(),
            expected: "Ожидался идентификатор.",
        },
        Test {
            name: "german with args",
            message: &X_0_expected,
            locale: locale::Locale::from("de-DE"),
            args: vec![Box::new(")")],
            expected: "\")\" wurde erwartet.",
        },
    ];

    for tt in tests {
        let result = tt.message.localize(tt.locale, tt.args);
        assert_eq!(result, tt.expected, "{}", tt.name);
    }
}

#[test]
fn test_localize_by_key() {
    struct Test {
        name: &'static str,
        key: Key,
        locale: locale::Locale,
        args: Vec<String>,
        expected: &'static str,
    }

    let tests = vec![
        Test {
            name: "by key without args",
            key: "Identifier_expected_1003".to_string(),
            locale: locale::Locale::from("en"),
            args: Vec::new(),
            expected: "Identifier expected.",
        },
        Test {
            name: "by key with args",
            key: "_0_expected_1005".to_string(),
            locale: locale::Locale::from("en"),
            args: vec![")".to_string()],
            expected: "')' expected.",
        },
    ];

    for tt in tests {
        let result = localize(tt.locale, None, tt.key, tt.args);
        assert_eq!(result, tt.expected, "{}", tt.name);
    }
}
