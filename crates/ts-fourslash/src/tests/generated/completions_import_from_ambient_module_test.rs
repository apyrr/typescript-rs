#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_from_ambient_module() {
    let mut t = TestingT;
    run_test_completions_import_from_ambient_module(&mut t);
}

fn run_test_completions_import_from_ambient_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: esnext
// @Filename: /a.ts
declare module "m" {
    export const x: number;
}
// @Filename: /b.ts
/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "x".to_string(),
            source: "m".to_string(),
            auto_import_fix: None,
            description: "Add import from \"m\"".to_string(),
            new_file_content: Some(
                r#"import { x } from "m";

"#
                .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
