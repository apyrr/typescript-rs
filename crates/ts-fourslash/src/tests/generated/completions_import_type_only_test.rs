#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_type_only() {
    let mut t = TestingT;
    run_test_completions_import_type_only(&mut t);
}

fn run_test_completions_import_type_only(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImport_typeOnly") {
        return;
    }
    let content = r"// @target: esnext
// @moduleResolution: bundler
// @Filename: /a.ts
export class A {}
export class B {}
// @Filename: /b.ts
import type { A } from './a';
const b: B/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "B".to_string(),
            source: "./a".to_string(),
            auto_import_fix: None,
            description: "Update import from \"./a\"".to_string(),
            new_file_content: Some(
                r"import type { A, B } from './a';
const b: B"
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
