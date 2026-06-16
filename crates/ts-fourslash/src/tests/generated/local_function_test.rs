#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_local_function() {
    let mut t = TestingT;
    run_test_local_function(&mut t);
}

fn run_test_local_function(t: &mut TestingT) {
    if should_skip_if_failing("TestLocalFunction") {
        return;
    }
    let content = r"function /*1*/foo() {
    function /*2*/bar2() {
    }
    var y = function /*3*/bar3() {
    }
}
var x = function /*4*/bar4() {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "function foo(): void", "");
    f.verify_quick_info_at(t, "2", "(local function) bar2(): void", "");
    f.verify_quick_info_at(t, "3", "(local function) bar3(): void", "");
    f.verify_quick_info_at(t, "4", "(local function) bar4(): void", "");
    done();
}
