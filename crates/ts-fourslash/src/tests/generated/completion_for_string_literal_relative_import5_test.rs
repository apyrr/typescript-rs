#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_relative_import5() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_relative_import5(&mut t);
}

fn run_test_completion_for_string_literal_relative_import5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @rootDirs: /repo/src1,/repo/src2/,/repo/generated1,/repo/generated2/
// @Filename: /dir/secret_file.ts
/*secret_file*/
// @Filename: /repo/src1/dir/test1.ts
import * as foo1 from ".//*import_as1*/
import foo2 = require(".//*import_equals1*/
var foo3 = require(".//*require1*/
// @Filename: /repo/src2/dir/test2.ts
import * as foo1 from "..//*import_as2*/
import foo2 = require("..//*import_equals2*/
var foo3 = require("..//*require2*/
// @Filename: /repo/src2/index.ts
import * as foo1 from ".//*import_as3*/
import foo2 = require(".//*import_equals3*/
var foo3 = require(".//*require3*/
// @Filename: /repo/generated1/dir/f1.ts
/*f1*/
// @Filename: /repo/generated2/dir/f2.ts
/*f2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "import_as1".to_string(),
            "import_equals1".to_string(),
            "require1".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("f1".to_string()),
                    CompletionsExpectedItem::Label("f2".to_string()),
                    CompletionsExpectedItem::Label("test2".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "import_as2".to_string(),
            "import_equals2".to_string(),
            "require2".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("dir".to_string()),
                    CompletionsExpectedItem::Label("index".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "import_as3".to_string(),
            "import_equals3".to_string(),
            "require3".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![CompletionsExpectedItem::Label("dir".to_string())],
            }),
            user_preferences: None,
        }),
    );
    done();
}
