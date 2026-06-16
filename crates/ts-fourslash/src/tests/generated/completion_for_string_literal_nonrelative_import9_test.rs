#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import9() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import9(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import9(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionForStringLiteralNonrelativeImport9") {
        return;
    }
    let content = r#"// @Filename: tsconfig.json
{
    "compilerOptions": {
        "baseUrl": "./modules",
        "paths": {
            "module1": ["some/path/whatever.ts"],
            "module2": ["some/other/path.ts"]
        }
    }
}
// @Filename: tests/test0.ts
import * as foo1 from "m/*import_as0*/
import foo2 = require("m/*import_equals0*/
var foo3 = require("m/*require0*/
// @Filename: some/path/whatever.ts
export var x = 9;
// @Filename: some/other/path.ts
export var y = 10;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("module1".to_string()),
                    CompletionsExpectedItem::Label("module2".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
