#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_no_hint_when_argument_matches_name() {
    let mut t = TestingT;
    run_test_inlay_hints_no_hint_when_argument_matches_name(&mut t);
}

fn run_test_inlay_hints_no_hint_when_argument_matches_name(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo (a: number, b: number) {}
declare const a: 1;
foo(a, 2);
declare const v: any;
foo(v.a, v.a);
foo(v.b, v.b);
foo(v.c, v.c);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
