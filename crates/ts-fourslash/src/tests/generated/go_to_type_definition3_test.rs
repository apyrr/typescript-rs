#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition3() {
    let mut t = TestingT;
    run_test_go_to_type_definition3(&mut t);
}

fn run_test_go_to_type_definition3(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToTypeDefinition3") {
        return;
    }
    let content = r"type /*definition*/T = string;
const x: /*reference*/T;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(t, &["reference".to_string()]);
    done();
}
