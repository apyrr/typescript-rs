use crate::{new_fourslash, InlayHintsPreferences, TestingT, UserPreferences};
use ts_lsproto as lsproto;

pub fn test_inlay_hints_interactive_parameter_names_in_span2(t: &mut TestingT) {
    let content = r#"function foo1 (a: number, b: number) {}
function foo2 (c: number, d: number) {}
function foo3 (e: number, f: number) {}
function foo4 (g: number, h: number) {}
function foo5 (i: number, j: number) {}
function foo6 (k: number, l: number) {}

foo1(/*a*/1, /*b*/2);
foo2(/*c*/1, /*d*/2);
foo3(/*e*/1, /*f*/2);
foo4(/*g*/1, /*h*/2);
foo5(/*i*/1, /*j*/2);
foo6(/*k*/1, /*l*/2);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let start = f.marker_by_name("c");
    let end = f.marker_by_name("h");
    let span = lsproto::Range {
        start: start.ls_position,
        end: end.ls_position,
    };
    f.verify_baseline_inlay_hints_with_preferences(
        t,
        Some(&span),
        &UserPreferences {
            inlay_hints: InlayHintsPreferences {
                include_inlay_parameter_name_hints: Some("literals".to_string()),
                ..InlayHintsPreferences::default()
            },
            ..UserPreferences::default()
        },
    );
    done();
}

