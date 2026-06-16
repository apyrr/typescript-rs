#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_verbatim_type_only1() {
    let mut t = TestingT;
    run_test_auto_import_verbatim_type_only1(&mut t);
}

fn run_test_auto_import_verbatim_type_only1(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportVerbatimTypeOnly1") {
        return;
    }
    let content = r"// @module: node18
// @verbatimModuleSyntax: true
// @Filename: /mod.ts
export const value = 0;
export class C { constructor(v: any) {} }
export interface I {}
// @Filename: /a.mts
const x: /**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "I".to_string(),
            source: "./mod".to_string(),
            auto_import_fix: Some(AutoImportFix),
            description: "Add import from \"./mod.js\"".to_string(),
            new_file_content: Some(
                r#"import type { I } from "./mod.js";

const x: "#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    f.insert(t, "I = new C");
    f.verify_apply_code_action_from_completion(
        t,
        None,
        &ApplyCodeActionFromCompletionOptions {
            name: "C".to_string(),
            source: "./mod".to_string(),
            auto_import_fix: Some(AutoImportFix),
            description: "Update import from \"./mod.js\"".to_string(),
            new_file_content: Some(
                r#"import { C, type I } from "./mod.js";

const x: I = new C"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
