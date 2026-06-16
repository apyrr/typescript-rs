#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_after_parameter() {
    let mut t = TestingT;
    run_test_signature_help_after_parameter(&mut t);
}

fn run_test_signature_help_after_parameter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type Type = (a, b, c) => void
const a: Type = (a/*1*/, b/*2*/) => {}
const b: Type = function (a/*3*/, b/*4*/) {}
const c: Type = ({ /*5*/a: { b/*6*/ }}/*7*/ = { }/*8*/, [b/*9*/]/*10*/, .../*11*/c/*12*/) => {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
