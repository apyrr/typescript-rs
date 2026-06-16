#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_object_literal_with_only_setter() {
    let mut t = TestingT;
    run_test_quick_info_on_object_literal_with_only_setter(&mut t);
}

fn run_test_quick_info_on_object_literal_with_only_setter(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnObjectLiteralWithOnlySetter") {
        return;
    }
    let content = r"function /*1*/makePoint(x: number) {
    return {
        b: 10,
        set x(a: number) { this.b = a; }
    };
};
var /*3*/point = makePoint(2);
point./*2*/x = 30;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "b".to_string(),
                        detail: Some("(property) b: number".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "x".to_string(),
                        detail: Some("(property) x: number".to_string()),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_quick_info_at(
        t,
        "1",
        "function makePoint(x: number): {\n    b: number;\n    x: number;\n}",
        "",
    );
    f.verify_quick_info_at(t, "2", "(property) x: number", "");
    f.verify_quick_info_at(
        t,
        "3",
        "var point: {\n    b: number;\n    x: number;\n}",
        "",
    );
    done();
}
