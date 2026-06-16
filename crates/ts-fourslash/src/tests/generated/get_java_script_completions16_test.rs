#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_completions16() {
    let mut t = TestingT;
    run_test_get_java_script_completions16(&mut t);
}

fn run_test_get_java_script_completions16(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJavaScriptCompletions16") {
        return;
    }
    let content = r#"// @allowNonTsExtensions: true
// @Filename: file.js
"use strict";

class Something {

    /**
     * @param {number} a
     */
    constructor(a, b) {
        a/*body*/
    }

    /**
     * @param {number} a
     */
    method(a) {
        a/*method*/
    }
}
let x = new Something(/*sig*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "body");
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
    f.go_to_marker(t, "sig");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("Something(a: number, b: any): Something".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "method");
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
    done();
}
