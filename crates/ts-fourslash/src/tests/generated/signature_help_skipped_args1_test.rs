#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_skipped_args1() {
    let mut t = TestingT;
    run_test_signature_help_skipped_args1(&mut t);
}

fn run_test_signature_help_skipped_args1(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpSkippedArgs1") {
        return;
    }
    let content = r"function fn(a: number, b: number, c: number) {}
fn(/*1*/, /*2*/, /*3*/, /*4*/, /*5*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
