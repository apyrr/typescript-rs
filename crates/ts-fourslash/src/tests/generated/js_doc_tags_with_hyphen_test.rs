#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_tags_with_hyphen() {
    let mut t = TestingT;
    run_test_js_doc_tags_with_hyphen(&mut t);
}

fn run_test_js_doc_tags_with_hyphen(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocTagsWithHyphen") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: dummy.js
/**
 * @typedef Product
 * @property {string} title
 * @property {boolean} h/*1*/igh-top some-comments
 */

/**
 * @type {Pro/*2*/duct}
 */
const product = {
    /*3*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) high-top: boolean", "some-comments");
    f.verify_quick_info_at(
        t,
        "2",
        "type Product = {\n    title: string;\n    \"high-top\": boolean;\n}",
        "",
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["3".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("\"high-top\"".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
