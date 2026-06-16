#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import17() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import17(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import17(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionForStringLiteralNonrelativeImport17") {
        return;
    }
    let content = r#"// @Filename: tsconfig.json
{
    "compilerOptions": {
        "paths": {
            "module1/*": ["some/path/*"],
        }
    }
}
// @Filename: test0.ts
import * as foo1 from "module1/w/*first*/
// @Filename: some/path/whatever.ts
export {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["first".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("whatever".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
