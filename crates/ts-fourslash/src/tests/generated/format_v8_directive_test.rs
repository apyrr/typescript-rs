#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_v8_directive() {
    let mut t = TestingT;
    run_test_format_v8_directive(&mut t);
}

fn run_test_format_v8_directive(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: foo.js
function foo() {}
/*1*/%PrepareFunctionForOptimization(foo)/*2*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "1", "2");
    done();
}
