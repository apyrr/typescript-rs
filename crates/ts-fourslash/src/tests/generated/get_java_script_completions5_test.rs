#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_completions5() {
    let mut t = TestingT;
    run_test_get_java_script_completions5(&mut t);
}

fn run_test_get_java_script_completions5(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJavaScriptCompletions5") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
/**
 * @template T
 * @param {T} a
 * @return {T} */
function foo(a) { }
let x = foo;
foo(1)./**/";
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
                    label: "toExponential".to_string(),
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
