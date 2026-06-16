#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_enum_members() {
    let mut t = TestingT;
    run_test_completion_list_enum_members(&mut t);
}

fn run_test_completion_list_enum_members(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"enum Foo {
    bar,
    baz
}

var v = Foo./*valueReference*/ba;
var t :Foo./*typeReference*/ba;
Foo.bar./*enumValueReference*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "valueReference".to_string(),
            "typeReference".to_string(),
        ]),
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
                    CompletionsExpectedItem::Label("bar".to_string()),
                    CompletionsExpectedItem::Label("baz".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("enumValueReference".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("toString".to_string()),
                    CompletionsExpectedItem::Label("toFixed".to_string()),
                    CompletionsExpectedItem::Label("toExponential".to_string()),
                    CompletionsExpectedItem::Label("toPrecision".to_string()),
                    CompletionsExpectedItem::Label("valueOf".to_string()),
                    CompletionsExpectedItem::Label("toLocaleString".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
