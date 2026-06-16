#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_yield_expression() {
    let mut t = TestingT;
    run_test_completions_import_yield_expression(&mut t);
}

fn run_test_completions_import_yield_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImportYieldExpression") {
        return;
    }
    let content = r"// @Filename: /a.ts
export function a() {}
// @Filename: /b.ts
function *f() {
  yield a/**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "a".to_string(),
            source: "./a".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./a\"".to_string(),
            new_file_content: Some(
                r#"import { a } from "./a";

function *f() {
  yield a
}"#
                .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
