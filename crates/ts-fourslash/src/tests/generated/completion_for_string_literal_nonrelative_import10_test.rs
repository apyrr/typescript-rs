#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import10() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import10(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import10(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionForStringLiteralNonrelativeImport10") {
        return;
    }
    let content = r#"// @moduleResolution: classic
// @Filename: dir1/dir2/dir3/dir4/test0.ts
import * as foo1 from "f/*import_as0*/
import * as foo3 from "fake-module/*import_as1*/
import foo4 = require("f/*import_equals0*/
import foo6 = require("fake-module/*import_equals1*/
var foo7 = require("f/*require0*/
var foo9 = require("fake-module/*require1*/
// @Filename: package.json
{ "dependencies": { "fake-module": "latest" } }
// @Filename: node_modules/fake-module/ts.ts

// @Filename: dir1/dir2/dir3/package.json
{ "dependencies": { "fake-module3": "latest" } }
// @Filename: dir1/dir2/dir3/node_modules/fake-module3/ts.ts
"#;
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
                exact: vec![],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
