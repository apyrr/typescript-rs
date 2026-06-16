#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_quick_info1() {
    let mut t = TestingT;
    run_test_get_java_script_quick_info1(&mut t);
}

fn run_test_get_java_script_quick_info1(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJavaScriptQuickInfo1") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
/** @type {function(new:string,number)} */
var /**/v;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var v: new (arg1: number) => string", "");
    done();
}
