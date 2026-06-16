#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_module_augmentation_with_js() {
    let mut t = TestingT;
    run_test_completions_import_module_augmentation_with_js(&mut t);
}

fn run_test_completions_import_module_augmentation_with_js(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @checkJs: true
// @noEmit: true
// @Filename: /test.js
class Abcde {
    x
}

module.exports = {
    Abcde
};
// @Filename: /index.ts
export {};
declare module "./test" {
    interface Abcde { b: string }
}

Abcde/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "Abcde".to_string(),
            source: "./test".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./test\"".to_string(),
            new_file_content: Some(
                r#"import { Abcde } from "./test";

export {};
declare module "./test" {
    interface Abcde { b: string }
}

Abcde"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
