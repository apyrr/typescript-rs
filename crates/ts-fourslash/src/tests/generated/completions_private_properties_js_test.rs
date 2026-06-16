#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_private_properties_js() {
    let mut t = TestingT;
    run_test_completions_private_properties_js(&mut t);
}

fn run_test_completions_private_properties_js(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsPrivateProperties_Js") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: a.d.ts
declare namespace A {
    class Foo {
        constructor();

        private m1(): void;
        protected m2(): void;

        m3(): void;
    }
}
// @filename: b.js
let foo = new A.Foo();
foo./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("m3".to_string())],
                excludes: vec!["m1".to_string(), "m2".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
