#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_lambda_this_members() {
    let mut t = TestingT;
    run_test_lambda_this_members(&mut t);
}

fn run_test_lambda_this_members(t: &mut TestingT) {
    if should_skip_if_failing("TestLambdaThisMembers") {
        return;
    }
    let content = r"class Foo {
    a: number;
    b() {
        var x = () => {
            this./**/;
        }
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
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
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
