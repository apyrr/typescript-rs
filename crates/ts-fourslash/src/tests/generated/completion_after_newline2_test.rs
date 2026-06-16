#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_after_newline2() {
    let mut t = TestingT;
    run_test_completion_after_newline2(&mut t);
}

fn run_test_completion_after_newline2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionAfterNewline2") {
        return;
    }
    let content = r"// @lib: es5
let foo = 5 as const /*1*/
/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("1".to_string()), None);
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
                exact: completion_globals_plus(
                    vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "foo".to_string(),
                        ..Default::default()
                    })],
                    false,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
