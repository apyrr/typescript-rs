#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_rest_args1() {
    let mut t = TestingT;
    run_test_signature_help_rest_args1(&mut t);
}

fn run_test_signature_help_rest_args1(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpRestArgs1") {
        return;
    }
    let content = r"function fn(a: number, b: number, c: number) {}
const a = [1, 2] as const;
const b = [1] as const;

fn(...a, /*1*/);
fn(/*2*/, ...a);

fn(...b, /*3*/);
fn(/*4*/, ...b, /*5*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
