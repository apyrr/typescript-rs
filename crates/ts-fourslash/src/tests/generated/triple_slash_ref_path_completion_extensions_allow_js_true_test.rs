#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_triple_slash_ref_path_completion_extensions_allow_js_true() {
    let mut t = TestingT;
    run_test_triple_slash_ref_path_completion_extensions_allow_js_true(&mut t);
}

fn run_test_triple_slash_ref_path_completion_extensions_allow_js_true(t: &mut TestingT) {
    if should_skip_if_failing("TestTripleSlashRefPathCompletionExtensionsAllowJSTrue") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: test0.ts
/// <reference path="/*0*/
/// <reference path=".//*1*/
/// <reference path="./f/*2*/
// @Filename: f1.ts

// @Filename: f1.js

// @Filename: f1.d.ts

// @Filename: f1.tsx

// @Filename: f1.js

// @Filename: f1.jsx

// @Filename: f1.cs
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("f1.d.ts".to_string()),
                    CompletionsExpectedItem::Label("f1.js".to_string()),
                    CompletionsExpectedItem::Label("f1.jsx".to_string()),
                    CompletionsExpectedItem::Label("f1.ts".to_string()),
                    CompletionsExpectedItem::Label("f1.tsx".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
