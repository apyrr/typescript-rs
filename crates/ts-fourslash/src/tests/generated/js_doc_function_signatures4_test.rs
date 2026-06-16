#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures4() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures4(&mut t);
}

fn run_test_js_doc_function_signatures4(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures4") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
/** @param {function ({OwnerID:string,AwayID:string}):void} x
  * @param {function (string):void} y */
function fn(x, y) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    done();
}
