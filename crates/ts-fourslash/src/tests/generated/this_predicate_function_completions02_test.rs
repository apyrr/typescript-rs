#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_this_predicate_function_completions02() {
    let mut t = TestingT;
    run_test_this_predicate_function_completions02(&mut t);
}

fn run_test_this_predicate_function_completions02(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Sundries {
    broken: boolean;
}

interface Supplies {
    spoiled: boolean;
}

interface Crate<T> {
    contents: T;
    isSundries(): this is Crate<Sundries>;
    isSupplies(): this is Crate<Supplies>;
    isPackedTight(): this is (this & {extraContents: T});
}
const crate: Crate<any>;
if (crate.isPackedTight()) {
    crate./*1*/;
}
if (crate.isSundries()) {
    crate.contents./*2*/;
    if (crate.isPackedTight()) {
        crate./*3*/;
    }
}
if (crate.isSupplies()) {
    crate.contents./*4*/;
    if (crate.isPackedTight()) {
        crate./*5*/;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "3".to_string(), "5".to_string()]),
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
                    CompletionsExpectedItem::Label("contents".to_string()),
                    CompletionsExpectedItem::Label("extraContents".to_string()),
                    CompletionsExpectedItem::Label("isPackedTight".to_string()),
                    CompletionsExpectedItem::Label("isSundries".to_string()),
                    CompletionsExpectedItem::Label("isSupplies".to_string()),
                ],
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
                exact: vec![CompletionsExpectedItem::Label("broken".to_string())],
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
                exact: vec![CompletionsExpectedItem::Label("spoiled".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
