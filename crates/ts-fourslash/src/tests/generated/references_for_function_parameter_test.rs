#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_function_parameter() {
    let mut t = TestingT;
    run_test_references_for_function_parameter(&mut t);
}

fn run_test_references_for_function_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForFunctionParameter") {
        return;
    }
    let content = r"var x;
var n;

function n(x: number, /*1*/n: number) {
    /*2*/n = 32;
    x = /*3*/n;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
