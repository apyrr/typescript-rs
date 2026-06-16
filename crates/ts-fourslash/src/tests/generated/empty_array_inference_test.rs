#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_empty_array_inference() {
    let mut t = TestingT;
    run_test_empty_array_inference(&mut t);
}

fn run_test_empty_array_inference(t: &mut TestingT) {
    if should_skip_if_failing("TestEmptyArrayInference") {
        return;
    }
    let content = r"// @strict: false
var x/*1*/x = true ? [1] : [undefined]; 
var y/*2*/y = true ? [1] : [];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var xx: number[]", "");
    f.verify_quick_info_at(t, "2", "var yy: number[]", "");
    done();
}
