#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures6() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures6(&mut t);
}

fn run_test_js_doc_function_signatures6(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures6") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: Foo.js
/**
 * @param {string} p1 - A string param
 * @param {string?} p2 - An optional param
 * @param {string} [p3] - Another optional param
 * @param {string} [p4="test"] - An optional param with a default value
 */
function f1(p1, p2, p3, p4){}
f1(/*1*/'foo', /*2*/'bar', /*3*/'baz', /*4*/'qux');"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
