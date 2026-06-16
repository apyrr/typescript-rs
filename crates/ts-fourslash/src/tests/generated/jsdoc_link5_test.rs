#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_link5() {
    let mut t = TestingT;
    run_test_jsdoc_link5(&mut t);
}

fn run_test_jsdoc_link5(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocLink5") {
        return;
    }
    let content = r"function g() { }
/**
 * {@link g()} {@link g() } {@link g ()} {@link g () 0} {@link g()1} {@link g() 2}
 * {@link u()} {@link u() } {@link u ()} {@link u () 0} {@link u()1} {@link u() 2}
 */
function f(x) {
}
f/*3*/()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
