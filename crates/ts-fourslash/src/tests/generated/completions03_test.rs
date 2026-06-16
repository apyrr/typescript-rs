#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions03() {
    let mut t = TestingT;
    run_test_completions03(&mut t);
}

fn run_test_completions03(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletions03") {
        return;
    }
    let content = r#"// @lib: es5
interface Foo {
   one: any;
   two: any;
   three: any;
}

let x: Foo = {
    get one() { return "" },
    set two(t) {},
    /**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
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
                exact: vec![CompletionsExpectedItem::Label("three".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
