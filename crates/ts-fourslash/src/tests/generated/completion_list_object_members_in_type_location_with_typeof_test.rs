#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_object_members_in_type_location_with_typeof() {
    let mut t = TestingT;
    run_test_completion_list_object_members_in_type_location_with_typeof(&mut t);
}

fn run_test_completion_list_object_members_in_type_location_with_typeof(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListObjectMembersInTypeLocationWithTypeof") {
        return;
    }
    let content = r"// @strict: true
const languageService = { getCompletions() {} }
type A = Parameters<typeof languageService./*1*/>

declare const obj: { dance: () => {} } | undefined
type B = Parameters<typeof obj./*2*/>";
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "getCompletions".to_string(),
                    detail: Some("(method) getCompletions(): void".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "dance".to_string(),
                    detail: Some("(property) dance: () => {})".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
