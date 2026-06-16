#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_type_literal_in_type_parameter1() {
    let mut t = TestingT;
    run_test_completion_list_in_type_literal_in_type_parameter1(&mut t);
}

fn run_test_completion_list_in_type_literal_in_type_parameter1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo {
    one: string;
    two: number;
    333: symbol;
    '4four': boolean;
    '5 five': object;
    number: string;
    Object: number;
}

interface Bar<T extends Foo> {
    foo: T;
}

var foobar: Bar<{/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("one".to_string()),
                    CompletionsExpectedItem::Label("two".to_string()),
                    CompletionsExpectedItem::Label("\"333\"".to_string()),
                    CompletionsExpectedItem::Label("\"4four\"".to_string()),
                    CompletionsExpectedItem::Label("\"5 five\"".to_string()),
                    CompletionsExpectedItem::Label("number".to_string()),
                    CompletionsExpectedItem::Label("Object".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
