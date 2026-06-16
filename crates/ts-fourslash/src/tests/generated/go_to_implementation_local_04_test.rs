#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_local_04() {
    let mut t = TestingT;
    run_test_go_to_implementation_local_04(&mut t);
}

fn run_test_go_to_implementation_local_04(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationLocal_04") {
        return;
    }
    let content = r"function [|he/*local_var*/llo|]() {}

hello();
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["local_var".to_string()]);
    done();
}
