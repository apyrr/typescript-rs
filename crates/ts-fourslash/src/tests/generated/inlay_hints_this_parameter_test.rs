#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_this_parameter() {
    let mut t = TestingT;
    run_test_inlay_hints_this_parameter(&mut t);
}

fn run_test_inlay_hints_this_parameter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    a: number;
}

declare function fn(
    callback: (a: number, b: string) => void
): void;


fn(function (this, a, b) { });
fn(function (this: I, a, b) { });";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
