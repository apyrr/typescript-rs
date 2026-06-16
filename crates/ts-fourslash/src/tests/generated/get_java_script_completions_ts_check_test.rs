#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_completions_ts_check() {
    let mut t = TestingT;
    run_test_get_java_script_completions_ts_check(&mut t);
}

fn run_test_get_java_script_completions_ts_check(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJavaScriptCompletions_tsCheck") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
// @ts-check
interface I { a: number; b: number; }
interface J { b: number; c: number; }
declare const ij: I | J;
ij./**/";
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
                exact: vec![CompletionsExpectedItem::Label("b".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
