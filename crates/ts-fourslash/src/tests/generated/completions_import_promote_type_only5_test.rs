#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_promote_type_only5() {
    let mut t = TestingT;
    run_test_completions_import_promote_type_only5(&mut t);
}

fn run_test_completions_import_promote_type_only5(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImport_promoteTypeOnly5") {
        return;
    }
    let content = r#"// @module: es2015
// @Filename: /exports.ts
export interface SomeInterface {}
export class SomePig {}
// @Filename: /a.ts
import { type SomePig as Babe } from "./exports.js";
new Babe/**/"#;
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
                    label: "Babe".to_string(),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "Babe".to_string(),
            source: "TypeOnlyAlias/".to_string(),
            auto_import_fix: None,
            description: "Remove 'type' from import of 'Babe' from \"./exports.js\"".to_string(),
            new_file_content: Some(
                r#"import { SomePig as Babe } from "./exports.js";
new Babe"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
