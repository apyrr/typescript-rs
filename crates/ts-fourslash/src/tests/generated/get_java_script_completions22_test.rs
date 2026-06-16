#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_completions22() {
    let mut t = TestingT;
    run_test_get_java_script_completions22(&mut t);
}

fn run_test_get_java_script_completions22(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowNonTsExtensions: true
// @Filename: file.js
const abc = {};
({./*1*/});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, ".");
    f.verify_completions(t, MarkerInput::None, None);
    done();
}
