#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_alias_quick_info() {
    let mut t = TestingT;
    run_test_js_doc_alias_quick_info(&mut t);
}

fn run_test_js_doc_alias_quick_info(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocAliasQuickInfo") {
        return;
    }
    let content = r#"// @Filename: /jsDocAliasQuickInfo.ts
/**
 * Comment
 * @type {number}
 */
export /*1*/default 10;
// @Filename: /test.ts
export { /*2*/default as /*3*/test } from "./jsDocAliasQuickInfo";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
