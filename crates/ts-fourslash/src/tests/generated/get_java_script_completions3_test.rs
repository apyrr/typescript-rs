#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_completions3() {
    let mut t = TestingT;
    run_test_get_java_script_completions3(&mut t);
}

fn run_test_get_java_script_completions3(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJavaScriptCompletions3") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
/** @type {Array.<number>} */
var v;
v./**/";
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "concat".to_string(),
                    kind: Some(lsproto::CompletionItemKind::METHOD),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
