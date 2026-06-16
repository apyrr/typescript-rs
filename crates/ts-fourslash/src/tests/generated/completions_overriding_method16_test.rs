#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_overriding_method16() {
    let mut t = TestingT;
    run_test_completions_overriding_method16(&mut t);
}

fn run_test_completions_overriding_method16(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @newline: LF
declare class B {
    set foo(value: any);
    get foo(): any;
}
class A extends B {
    /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_completions(t, &[]);
    done();
}
