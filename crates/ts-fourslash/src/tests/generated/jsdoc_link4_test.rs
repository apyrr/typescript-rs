#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_link4() {
    let mut t = TestingT;
    run_test_jsdoc_link4(&mut t);
}

fn run_test_jsdoc_link4(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocLink4") {
        return;
    }
    let content = r"declare class I {
  /** {@link I} */
  bar/*1*/(): void
}
/** {@link I} */
var n/*2*/ = 1
/**
 * A real, very serious {@link I to an interface}. Right there.
 * @param x one {@link Pos here too}
 */
function f(x) {
}
f/*3*/()
type Pos = [number, number]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
