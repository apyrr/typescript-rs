#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_relative_import_allow_js_true() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_relative_import_allow_js_true(&mut t);
}

fn run_test_completion_for_string_literal_relative_import_allow_js_true(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @Filename: test0.ts
import * as foo1 from ".//*import_as0*/
import * as foo2 from "./f/*import_as1*/
import foo3 = require(".//*import_equals0*/
import foo4 = require("./f/*import_equals1*/
var foo5 = require(".//*require0*/
var foo6 = require("./f/*require1*/
// @Filename: f1.ts

// @Filename: f2.js

// @Filename: f3.d.ts

// @Filename: f4.tsx

// @Filename: f5.js

// @Filename: f6.jsx

// @Filename: g1.ts

// @Filename: g2.js
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
                    CompletionsExpectedItem::Label("f1".to_string()),
                    CompletionsExpectedItem::Label("f2".to_string()),
                    CompletionsExpectedItem::Label("f3".to_string()),
                    CompletionsExpectedItem::Label("f4".to_string()),
                    CompletionsExpectedItem::Label("f5".to_string()),
                    CompletionsExpectedItem::Label("f6".to_string()),
                    CompletionsExpectedItem::Label("g1".to_string()),
                    CompletionsExpectedItem::Label("g2".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
