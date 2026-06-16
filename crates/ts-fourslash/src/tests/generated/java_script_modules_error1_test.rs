#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_java_script_modules_error1() {
    let mut t = TestingT;
    run_test_java_script_modules_error1(&mut t);
}

fn run_test_java_script_modules_error1(t: &mut TestingT) {
    if should_skip_if_failing("TestJavaScriptModulesError1") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
define('mod1', ['a'], /**/function(a, b) {
	
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    done();
}
