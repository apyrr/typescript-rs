#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import_typings2() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import_typings2(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import_typings2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @typeRoots: my_typings,my_other_typings
// @types: module-x,module-z
// @Filename: tests/test0.ts
/// <reference types="m/*types_ref0*/" />
import * as foo1 from "m/*import_as0*/
import foo2 = require("m/*import_equals0*/
var foo3 = require("m/*require0*/
// @Filename: my_typings/module-x/index.d.ts
export var x = 9;
// @Filename: my_typings/module-y/index.d.ts
export var y = 9;
// @Filename: my_other_typings/module-z/index.d.ts
export var z = 9;"#;
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
                    CompletionsExpectedItem::Label("module-x".to_string()),
                    CompletionsExpectedItem::Label("module-z".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
