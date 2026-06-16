#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_unclosed_object_type_literal_in_signature02() {
    let mut t = TestingT;
    run_test_completion_list_in_unclosed_object_type_literal_in_signature02(&mut t);
}

fn run_test_completion_list_in_unclosed_object_type_literal_in_signature02(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInUnclosedObjectTypeLiteralInSignature02") {
        return;
    }
    let content = r"interface I<TString, TNumber> {
    [s: string]: TString;
    [s: number]: TNumber;
}

declare function foo<TString, TNumber>(obj: I<TString, TNumber>): { str: TStr/*1*/";
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
                includes: vec![
                    CompletionsExpectedItem::Label("I".to_string()),
                    CompletionsExpectedItem::Label("TString".to_string()),
                    CompletionsExpectedItem::Label("TNumber".to_string()),
                ],
                excludes: vec!["foo".to_string(), "obj".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
