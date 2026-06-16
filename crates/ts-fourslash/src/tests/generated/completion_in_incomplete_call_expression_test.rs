#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_in_incomplete_call_expression() {
    let mut t = TestingT;
    run_test_completion_in_incomplete_call_expression(&mut t);
}

fn run_test_completion_in_incomplete_call_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionInIncompleteCallExpression") {
        return;
    }
    let content = r"// @lib: es5
var array = [1, 2, 4]
function a4(x, y, z) { }
a4(...<crash>/**/";
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
                exact: completion_globals_plus(
                    vec![
                        CompletionsExpectedItem::Label("a4".to_string()),
                        CompletionsExpectedItem::Label("array".to_string()),
                    ],
                    false,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
