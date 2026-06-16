#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_object_spread() {
    let mut t = TestingT;
    run_test_go_to_definition_object_spread(&mut t);
}

fn run_test_go_to_definition_object_spread(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface A1 { /*1*/a: number };
interface A2 { /*2*/a?: number };
let a1: A1;
let a2: A2;
let a12 = { ...a1, ...a2 };
a12.[|a/*3*/|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["3".to_string()]);
    done();
}
