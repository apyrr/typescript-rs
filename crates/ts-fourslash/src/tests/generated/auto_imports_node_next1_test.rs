#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_imports_node_next1() {
    let mut t = TestingT;
    run_test_auto_imports_node_next1(&mut t);
}

fn run_test_auto_imports_node_next1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: node18
// @Filename: /node_modules/pack/package.json
{
    "name": "pack",
    "version": "1.0.0",
    "exports": {
        ".": "./main.mjs"
    }
}
// @Filename: /node_modules/pack/main.d.mts
import {} from "./unreachable.mjs";
export const fromMain = 0;
// @Filename: /node_modules/pack/unreachable.d.mts
export const fromUnreachable = 0;
// @Filename: /index.mts
import { fromMain } from "pack";
fromUnreachable/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(t, &[], None);
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
                excludes: vec!["fromUnreachable".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
