#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_with_override2() {
    let mut t = TestingT;
    run_test_completions_with_override2(&mut t);
}

fn run_test_completions_with_override2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsWithOverride2") {
        return;
    }
    let content = r"interface I {
    baz () {}
}
class A {
    foo () {} 
    bar () {}
}
class B extends A implements I {
    override /*1*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
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
                includes: vec![
                    CompletionsExpectedItem::Label("foo".to_string()),
                    CompletionsExpectedItem::Label("bar".to_string()),
                ],
                excludes: vec!["baz".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
