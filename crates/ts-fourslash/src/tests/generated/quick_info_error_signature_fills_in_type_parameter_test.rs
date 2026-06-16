#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_error_signature_fills_in_type_parameter() {
    let mut t = TestingT;
    run_test_quick_info_error_signature_fills_in_type_parameter(&mut t);
}

fn run_test_quick_info_error_signature_fills_in_type_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfo_errorSignatureFillsInTypeParameter") {
        return;
    }
    let content = r"declare function f<T>(x: number): T;
const x/**/ = f();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "const x: unknown", "");
    done();
}
