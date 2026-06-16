#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_rest_parameters3() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_rest_parameters3(&mut t);
}

fn run_test_inlay_hints_interactive_rest_parameters3(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsInteractiveRestParameters3") {
        return;
    }
    let content = r"function fn(x: number, y: number, a: number, b: number) {
    return x + y + a + b;
}
const foo: [x: number, y: number] = [1, 2];
fn(...foo, 3, 4);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
