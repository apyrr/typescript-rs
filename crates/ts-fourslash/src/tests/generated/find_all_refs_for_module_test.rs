#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_module() {
    let mut t = TestingT;
    run_test_find_all_refs_for_module(&mut t);
}

fn run_test_find_all_refs_for_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @Filename: /a.ts
export const x = 0;
// @Filename: /b.ts
[|import { x } from "/*0*/[|{| "contextRangeIndex": 0 |}./a|]";|]
// @Filename: /c/sub.js
[|const a = require("/*1*/[|{| "contextRangeIndex": 2 |}../a|]");|]
// @Filename: /d.ts
 /// <reference path="/*2*/[|./a.ts|]" />"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    f.verify_baseline_document_highlights_with_options(
        t,
        None,
        vec![
            "/b.ts".to_string(),
            "/c/sub.js".to_string(),
            "/d.ts".to_string(),
        ],
        vec![
            MarkerOrRangeOrName::Range(f.ranges()[1].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[3].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[4].clone()),
        ],
    );
    done();
}
