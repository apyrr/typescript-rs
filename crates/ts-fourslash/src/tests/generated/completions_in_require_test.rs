#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_in_require() {
    let mut t = TestingT;
    run_test_completions_in_require(&mut t);
}

fn run_test_completions_in_require(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsInRequire") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: foo.js
var foo = require("/**/"

foo();

/**
 * @return {void}
 */
function foo() {
}
// @Filename: package.json
 { "dependencies": { "fake-module": "latest" } }
// @Filename: node_modules/fake-module/index.js
/* fake-module */"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("fake-module".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
