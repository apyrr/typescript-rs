#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_in_type_of1() {
    let mut t = TestingT;
    run_test_completion_in_type_of1(&mut t);
}

fn run_test_completion_in_type_of1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionInTypeOf1") {
        return;
    }
    let content = r"namespace m1c {
    export interface I { foo(): void; }
}
var x: typeof m1c./*1*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("1".to_string()), None);
    done();
}
