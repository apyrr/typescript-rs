#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures3() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures3(&mut t);
}

fn run_test_js_doc_function_signatures3(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures3") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
var someObject = {
    /**
     * @param {string} param1 Some string param.
     * @param {number} parm2  Some number param.
     */
    someMethod: function(param1, param2) {
        console.log(param1/*1*/);
        return false;
    },
    /**
     * @param {number} p1  Some number param.
     */
    otherMethod(p1) {
        p1/*2*/
    }

};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, ".");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "substring".to_string(),
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
    f.backspace(t, 1);
    f.go_to_marker(t, "2");
    f.insert(t, ".");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "toFixed".to_string(),
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
    f.backspace(t, 1);
    done();
}
