#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_global_this_completion() {
    let mut t = TestingT;
    run_test_global_this_completion(&mut t);
}

fn run_test_global_this_completion(t: &mut TestingT) {
    if should_skip_if_failing("TestGlobalThisCompletion") {
        return;
    }
    let content = r#"// @allowJs: true
// @target: esnext
// @Filename: test.js
(typeof foo !== "undefined"
  ? foo
  : {}
)./**/;
// @Filename: someLib.d.ts
declare var foo: typeof globalThis;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
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
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
