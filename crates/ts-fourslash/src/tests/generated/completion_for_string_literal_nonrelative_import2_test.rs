#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import2() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import2(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionForStringLiteralNonrelativeImport2") {
        return;
    }
    let content = r#"// @Filename: tests/test0.ts
import * as foo1 from "fake-module//*import_as0*/
import foo2 = require("fake-module//*import_equals0*/
var foo3 = require("fake-module//*require0*/
// @Filename: package.json
{ "dependencies": { "fake-module": "latest" }, "devDependencies": { "fake-module-dev": "latest" } }
// @Filename: node_modules/fake-module/repeated.ts
/*repeatedts*/
// @Filename: node_modules/fake-module/repeated.tsx
/*repeatedtsx*/
// @Filename: node_modules/fake-module/repeated.d.ts
/*repeateddts*/
// @Filename: node_modules/fake-module/other.js
/*other*/
// @Filename: node_modules/fake-module/other2.js
/*other2*/
// @Filename: node_modules/unlisted-module/index.js
/*unlisted-module*/
// @Filename: ambient.ts
declare module "fake-module/other""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "import_as0".to_string(),
            "import_equals0".to_string(),
            "require0".to_string(),
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
                exact: vec![
                    CompletionsExpectedItem::Label("other".to_string()),
                    CompletionsExpectedItem::Label("repeated".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
