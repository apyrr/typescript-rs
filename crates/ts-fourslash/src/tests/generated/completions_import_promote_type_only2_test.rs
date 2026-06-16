#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_promote_type_only2() {
    let mut t = TestingT;
    run_test_completions_import_promote_type_only2(&mut t);
}

fn run_test_completions_import_promote_type_only2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: es2015
// @Filename: /exports.ts
export interface SomeInterface {}
// @Filename: /a.ts
import type { SomeInterface } from "./exports.js";
const SomeInterface = {};
SomeI/**/"#;
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
                    label: "SomeInterface".to_string(),
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
