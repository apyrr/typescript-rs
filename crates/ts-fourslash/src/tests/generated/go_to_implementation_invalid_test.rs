#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_invalid() {
    let mut t = TestingT;
    run_test_go_to_implementation_invalid(&mut t);
}

fn run_test_go_to_implementation_invalid(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInvalid") {
        return;
    }
    let content = r#"var x1 = 50/*0*/0;
var x2 = "hel/*1*/lo";
/*2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    done();
}
