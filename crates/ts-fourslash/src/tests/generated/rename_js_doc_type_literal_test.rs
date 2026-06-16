#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_doc_type_literal() {
    let mut t = TestingT;
    run_test_rename_js_doc_type_literal(&mut t);
}

fn run_test_rename_js_doc_type_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJsDocTypeLiteral") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @filename: /a.js
/**
 * @param {Object} options
 * @param {string} options.foo
 * @param {number} options.bar
 */
function foo(/**/options) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.js");
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
