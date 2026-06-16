#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_import_tag_completion3() {
    let mut t = TestingT;
    run_test_jsdoc_import_tag_completion3(&mut t);
}

fn run_test_jsdoc_import_tag_completion3(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocImportTagCompletion3") {
        return;
    }
    let content = r#"// @allowJS: true
// @checkJs: true
// @module: esnext
// @filename: ./a.ts
export interface A {}
// @filename: ./b.ts
export interface B {}
// @filename: ./c.js
/**
 * @import * as types from ".//**/"
 */"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
