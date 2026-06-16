#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_on_super() {
    let mut t = TestingT;
    run_test_completion_list_on_super(&mut t);
}

fn run_test_completion_list_on_super(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListOnSuper") {
        return;
    }
    let content = r"class TAB<T>{
    foo<T>(x: T) {
    }
    bar(a: number, b: number) {
    }
}

class TAD<T> extends TAB<T> {
    constructor() {
        super();
    }
    bar(f: number) {
        super./**/
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
                    CompletionsExpectedItem::Label("bar".to_string()),
                    CompletionsExpectedItem::Label("foo".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
