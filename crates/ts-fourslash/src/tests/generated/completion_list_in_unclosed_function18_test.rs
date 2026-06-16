#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_unclosed_function18() {
    let mut t = TestingT;
    run_test_completion_list_in_unclosed_function18(&mut t);
}

fn run_test_completion_list_in_unclosed_function18(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInUnclosedFunction18") {
        return;
    }
    let content = r#"interface MyType {
}

function foo(x: string, y: number, z: boolean) {
    function bar(a: number, b: string = "hello", c: typeof x = "hello") {
        var v = (p: MyType) => y + /*1*/
}"#;
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
                    CompletionsExpectedItem::Label("foo".to_string()),
                    CompletionsExpectedItem::Label("x".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                    CompletionsExpectedItem::Label("z".to_string()),
                    CompletionsExpectedItem::Label("bar".to_string()),
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("c".to_string()),
                    CompletionsExpectedItem::Label("v".to_string()),
                    CompletionsExpectedItem::Label("p".to_string()),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
