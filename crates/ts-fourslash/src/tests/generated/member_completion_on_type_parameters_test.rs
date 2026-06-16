#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_completion_on_type_parameters() {
    let mut t = TestingT;
    run_test_member_completion_on_type_parameters(&mut t);
}

fn run_test_member_completion_on_type_parameters(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface IFoo {
    x: number;
    y: string;
}

function foo<S, T extends IFoo, U extends Object, V extends IFoo>() {
    var s:S, t: T, u: U, v: V;
    s./*S*/;    // no constraint, no completion
    t./*T*/;    // IFoo
    u./*U*/;    // IFoo
    v./*V*/;    // IFoo
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("S".to_string()), None);
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["T".to_string(), "V".to_string()]),
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
                        label: "x".to_string(),
                        detail: Some("(property) IFoo.x: number".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "y".to_string(),
                        detail: Some("(property) IFoo.y: string".to_string()),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("U".to_string()),
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
                    CompletionsExpectedItem::Label("constructor".to_string()),
                    CompletionsExpectedItem::Label("toString".to_string()),
                    CompletionsExpectedItem::Label("toLocaleString".to_string()),
                    CompletionsExpectedItem::Label("valueOf".to_string()),
                    CompletionsExpectedItem::Label("hasOwnProperty".to_string()),
                    CompletionsExpectedItem::Label("isPrototypeOf".to_string()),
                    CompletionsExpectedItem::Label("propertyIsEnumerable".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
