#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_module_variables() {
    let mut t = TestingT;
    run_test_quick_info_module_variables(&mut t);
}

fn run_test_quick_info_module_variables(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var x = 1;
namespace M {
    export var x = 2;
    console.log(/*1*/x); // 2
}
namespace M {
    console.log(/*2*/x); // 2
}
namespace M {
    var x = 3;
    console.log(/*3*/x); // 3
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var M.x: number", "");
    f.verify_quick_info_at(t, "2", "var M.x: number", "");
    f.verify_quick_info_at(t, "3", "var x: number", "");
    done();
}
