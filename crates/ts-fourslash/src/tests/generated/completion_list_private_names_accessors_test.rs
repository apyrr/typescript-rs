#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_private_names_accessors() {
    let mut t = TestingT;
    run_test_completion_list_private_names_accessors(&mut t);
}

fn run_test_completion_list_private_names_accessors(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListPrivateNamesAccessors") {
        return;
    }
    let content = r"class Foo {
   get #x() { return 1 };
   set #x(value: number) { };
   y() {};
}
class Bar extends Foo {
   get #z() { return 1 };
   set #z(value: number) { };
   t() {};
   l;
   constructor() {
       this./*1*/
       class Baz {
           get #z() { return 1 };
           set #z(value: number) { };
           get #u() { return 1 };
           set #u(value: number) { };
           v() {};
           k;
           constructor() {
               this./*2*/
               new Bar()./*3*/
           }
       }
   }
}

new Foo()./*4*/";
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
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("#z".to_string()),
                    CompletionsExpectedItem::Label("t".to_string()),
                    CompletionsExpectedItem::Label("l".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                ],
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
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("#z".to_string()),
                    CompletionsExpectedItem::Label("#u".to_string()),
                    CompletionsExpectedItem::Label("v".to_string()),
                    CompletionsExpectedItem::Label("k".to_string()),
                ],
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
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("#z".to_string()),
                    CompletionsExpectedItem::Label("t".to_string()),
                    CompletionsExpectedItem::Label("l".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                ],
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
                exact: vec![CompletionsExpectedItem::Label("y".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
