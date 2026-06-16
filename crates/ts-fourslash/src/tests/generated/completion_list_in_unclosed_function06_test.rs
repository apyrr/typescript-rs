#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_unclosed_function06() {
    let mut t = TestingT;
    run_test_completion_list_in_unclosed_function06(&mut t);
}

fn run_test_completion_list_in_unclosed_function06(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function foo(x: string, y: number, z: boolean) {
    function bar(a: number, b: string = /*1*/, c: typeof x = "hello"
"#;
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
