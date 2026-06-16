#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_details_with_misspelled_name() {
    let mut t = TestingT;
    run_test_completions_import_details_with_misspelled_name(&mut t);
}

fn run_test_completions_import_details_with_misspelled_name(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImport_details_withMisspelledName") {
        return;
    }
    let content = r"// @Filename: /a.ts
export const abc = 0;
// @Filename: /b.ts
acb/*1*/;
// @Filename: /c.ts
acb/*2*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_apply_code_action_from_completion(
        t,
        Some("1"),
        &ApplyCodeActionFromCompletionOptions {
            name: "abc".to_string(),
            source: "./a".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./a\"".to_string(),
            new_file_content: Some(
                r#"import { abc } from "./a";

acb;"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_apply_code_action_from_completion(
        t,
        Some("2"),
        &ApplyCodeActionFromCompletionOptions {
            name: "abc".to_string(),
            source: "./a".to_string(),
            auto_import_fix: Some(AutoImportFix),
            description: "Add import from \"./a\"".to_string(),
            new_file_content: Some(
                r#"import { abc } from "./a";

acb;"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
