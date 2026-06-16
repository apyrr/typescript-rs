#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_for_derived_type1() {
    let mut t = TestingT;
    run_test_completion_list_for_derived_type1(&mut t);
}

fn run_test_completion_list_for_derived_type1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListForDerivedType1") {
        return;
    }
    let content = r"interface IFoo {
    bar(): IFoo;
}
interface IFoo2 extends IFoo {
    bar2(): IFoo2;
}
var f: IFoo;
var f2: IFoo2;
f./*1*/; // completion here shows bar with return type is any
f2./*2*/ // here bar has return type any, but bar2 is Foo2";
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
                exact: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "bar".to_string(),
                    detail: Some("(method) IFoo.bar(): IFoo".to_string()),
                    ..Default::default()
                })],
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
                exact: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "bar".to_string(),
                        detail: Some("(method) IFoo.bar(): IFoo".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "bar2".to_string(),
                        detail: Some("(method) IFoo2.bar2(): IFoo2".to_string()),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
