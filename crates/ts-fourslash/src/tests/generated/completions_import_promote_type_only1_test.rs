#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_promote_type_only1() {
    let mut t = TestingT;
    run_test_completions_import_promote_type_only1(&mut t);
}

fn run_test_completions_import_promote_type_only1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImport_promoteTypeOnly1") {
        return;
    }
    let content = r#"// @lib: es5
// @module: es2015
// @Filename: /exports.ts
export interface SomeInterface {}
export class SomePig {}
// @Filename: /a.ts
import type { SomeInterface, SomePig } from "./exports.js";
new SomePig/**/"#;
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals_plus(
                    vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "SomePig".to_string(),
                        ..Default::default()
                    })],
                    false,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "SomePig".to_string(),
            source: "TypeOnlyAlias/".to_string(),
            auto_import_fix: None,
            description: "Remove 'type' from import declaration from \"./exports.js\"".to_string(),
            new_file_content: Some(
                r#"import { SomeInterface, SomePig } from "./exports.js";
new SomePig"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
