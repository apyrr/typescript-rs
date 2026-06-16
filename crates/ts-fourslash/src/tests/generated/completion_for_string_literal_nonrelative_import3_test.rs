#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import3() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import3(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @Filename: tests/test0.ts
import * as foo1 from "fake-module//*import_as0*/
import foo2 = require("fake-module//*import_equals0*/
var foo3 = require("fake-module//*require0*/
// @Filename: package.json
{ "dependencies": { "fake-module": "latest" } }
// @Filename: node_modules/fake-module/ts.ts
/*ts*/
// @Filename: node_modules/fake-module/tsx.tsx
/*tsx*/
// @Filename: node_modules/fake-module/dts.d.ts
/*dts*/
// @Filename: node_modules/fake-module/js.js
/*js*/
// @Filename: node_modules/fake-module/jsx.jsx
/*jsx*/
// @Filename: node_modules/fake-module/repeated.js
/*repeatedjs*/
// @Filename: node_modules/fake-module/repeated.jsx
/*repeatedjsx*/"#;
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
                    CompletionsExpectedItem::Label("dts".to_string()),
                    CompletionsExpectedItem::Label("js".to_string()),
                    CompletionsExpectedItem::Label("jsx".to_string()),
                    CompletionsExpectedItem::Label("repeated".to_string()),
                    CompletionsExpectedItem::Label("ts".to_string()),
                    CompletionsExpectedItem::Label("tsx".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
