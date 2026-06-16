#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_rest_parameters2() {
    let mut t = TestingT;
    run_test_inlay_hints_rest_parameters2(&mut t);
}

fn run_test_inlay_hints_rest_parameters2(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsRestParameters2") {
        return;
    }
    let content = r"function foo(a: unknown, b: unknown, c: unknown) { }
function foo1(...x: [number, number | undefined]) {
    foo(...x, 3);
}
function foo2(...x: []) {
    foo(...x, 1, 2, 3);
}
function foo3(...x: [number, number?]) {
    foo(1, ...x);
}
function foo4(...x: [number, number?]) {
    foo(...x, 3);
}
function foo5(...x: [number, number]) {
    foo(...x, 3);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
