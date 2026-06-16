#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_function_parameter_types5() {
    let mut t = TestingT;
    run_test_inlay_hints_function_parameter_types5(&mut t);
}

fn run_test_inlay_hints_function_parameter_types5(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsFunctionParameterTypes5") {
        return;
    }
    let content = r"declare const STATE_SIGNAL: unique symbol;

declare function test(
  cb: (state: { [STATE_SIGNAL]: unknown }) => void,
): unknown;

test((state) => {});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
