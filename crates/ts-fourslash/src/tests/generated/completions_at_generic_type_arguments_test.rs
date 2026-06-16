#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_at_generic_type_arguments() {
    let mut t = TestingT;
    run_test_completions_at_generic_type_arguments(&mut t);
}

fn run_test_completions_at_generic_type_arguments(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsAtGenericTypeArguments") {
        return;
    }
    let content = r"// @lib: es5
class Foo<T1, T2> {}
const foo = new Foo</*1*/, /*2*/,

function foo<T1, T2>() {}
const f = foo</*3*/, /*4*/,";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_global_types_plus(vec![CompletionsExpectedItem::Label(
                    "Foo".to_string(),
                )]),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
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
                exact: completion_global_types_plus(vec![CompletionsExpectedItem::Label(
                    "Foo".to_string(),
                )]),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("3".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_global_types_plus(vec![CompletionsExpectedItem::Label(
                    "Foo".to_string(),
                )]),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("4".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_global_types_plus(vec![CompletionsExpectedItem::Label(
                    "Foo".to_string(),
                )]),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
