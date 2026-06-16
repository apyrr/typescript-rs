#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_any_parameter1() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_any_parameter1(&mut t);
}

fn run_test_inlay_hints_interactive_any_parameter1(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsInteractiveAnyParameter1") {
        return;
    }
    let content = r"function foo (v: any) {}
foo(1);
foo('');
foo(true);
foo(foo);
foo((1));
foo(foo(1));";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
