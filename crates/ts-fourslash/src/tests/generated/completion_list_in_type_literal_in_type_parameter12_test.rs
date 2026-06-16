#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_type_literal_in_type_parameter12() {
    let mut t = TestingT;
    run_test_completion_list_in_type_literal_in_type_parameter12(&mut t);
}

fn run_test_completion_list_in_type_literal_in_type_parameter12(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInTypeLiteralInTypeParameter12") {
        return;
    }
    let content = r"interface Foo {
    kind: 'foo';
    one: string;
}
interface Bar {
    kind: 'bar';
    two: number;
}

declare function a<T extends Foo>(): void
declare function a<T extends Bar>(): void
a<{ kind: 'bar', /*0*/ }>();

declare function b<T extends Foo>(kind: 'foo'): void
declare function b<T extends Bar>(kind: 'bar'): void
b<{/*1*/}>('bar');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("0".to_string()),
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
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
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
                    CompletionsExpectedItem::Label("kind".to_string()),
                    CompletionsExpectedItem::Label("one".to_string()),
                    CompletionsExpectedItem::Label("two".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
