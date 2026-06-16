#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_quote_style() {
    let mut t = TestingT;
    run_test_completions_import_quote_style(&mut t);
}

fn run_test_completions_import_quote_style(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImport_quoteStyle") {
        return;
    }
    let content = r"// @module: esnext
// @Filename: /a.ts
export const foo = 0;
// @Filename: /b.ts
fo/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "foo".to_string(),
            source: "./a".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./a\"".to_string(),
            new_file_content: Some(
                r"import { foo } from './a';

fo"
                .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
