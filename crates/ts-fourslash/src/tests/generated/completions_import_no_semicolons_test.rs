#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_no_semicolons() {
    let mut t = TestingT;
    run_test_completions_import_no_semicolons(&mut t);
}

fn run_test_completions_import_no_semicolons(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /a.ts
export function foo() {}
// @Filename: /b.ts
const x = 0
const y = 1
const z = fo/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "foo".to_string(),
            source: "./a".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./a\"".to_string(),
            new_file_content: Some(
                r#"import { foo } from "./a"

const x = 0
const y = 1
const z = fo"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
