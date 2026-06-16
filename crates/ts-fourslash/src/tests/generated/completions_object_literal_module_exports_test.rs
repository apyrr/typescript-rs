#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_object_literal_module_exports() {
    let mut t = TestingT;
    run_test_completions_object_literal_module_exports(&mut t);
}

fn run_test_completions_object_literal_module_exports(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsObjectLiteralModuleExports") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: index.js
const almanac = 0;
module.exports = {
  a/**/
};";
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
                includes: vec![CompletionsExpectedItem::Label("almanac".to_string())],
                excludes: vec!["a".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
