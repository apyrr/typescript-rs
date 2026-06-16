#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_self_referenced_external_module2() {
    let mut t = TestingT;
    run_test_self_referenced_external_module2(&mut t);
}

fn run_test_self_referenced_external_module2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: app.ts
export import A = require('./app2');
export var I = 1;
A./*1*/Y;
A.B.A.B./*2*/I;
// @Filename: app2.ts
export import B = require('./app');
export var Y = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var A.Y: number", "");
    f.verify_quick_info_at(t, "2", "var I: number", "");
    done();
}
