#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_literal_completions_in_jsx_attribute_initializer() {
    let mut t = TestingT;
    run_test_string_literal_completions_in_jsx_attribute_initializer(&mut t);
}

fn run_test_string_literal_completions_in_jsx_attribute_initializer(t: &mut TestingT) {
    if should_skip_if_failing("TestStringLiteralCompletionsInJsxAttributeInitializer") {
        return;
    }
    let content = r#"// @jsx: preserve
// @filename: /a.tsx
type Props = { a: number } | { b: "somethingelse", c: 0 | 1 };
declare function Foo(args: Props): any

const a1 = <Foo b={"/*1*/"} />
const a2 = <Foo b="/*2*/" />
const a3 = <Foo b="somethingelse"/*3*/ />
const a4 = <Foo b={"somethingelse"} /*4*/ />
const a5 = <Foo b={"somethingelse"} c={0} /*5*/ />"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "2".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("somethingelse".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["3".to_string(), "4".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec!["\"somethingelse\"".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["5".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec!["0".to_string(), "1".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
