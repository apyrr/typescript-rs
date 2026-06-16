#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import7() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import7(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import7(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @baseUrl: tests/cases/fourslash/modules
// @Filename: tests/test0.ts
import * as foo1 from "mod/*import_as0*/
import foo2 = require("mod/*import_equals0*/
var foo3 = require("mod/*require0*/
// @Filename: modules/module.ts
export var x = 5;
// @Filename: package.json
{ "dependencies": { "module-from-node": "latest" } }
// @Filename: node_modules/module-from-node/index.ts
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
                exact: vec![
                    CompletionsExpectedItem::Label("module".to_string()),
                    CompletionsExpectedItem::Label("module-from-node".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
