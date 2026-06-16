#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_return_with_contextual_this() {
    let mut t = TestingT;
    run_test_completion_list_in_return_with_contextual_this(&mut t);
}

fn run_test_completion_list_in_return_with_contextual_this(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInReturnWithContextualThis") {
        return;
    }
    let content = r"interface Ctx {
    foo(): {
        x: number
    };
}

declare function wrap(cb: (this: Ctx) => any): void;

wrap(function () {
    const xs = this.foo();
    return xs./*inReturn*/
});

wrap(function () {
    const xs = this.foo();
    const y = xs./*involvedInReturn*/
    return y;
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("inReturn".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("x".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("involvedInReturn".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("x".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
