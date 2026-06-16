#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_class_method_00() {
    let mut t = TestingT;
    run_test_go_to_implementation_class_method_00(&mut t);
}

fn run_test_go_to_implementation_class_method_00(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationClassMethod_00") {
        return;
    }
    let content = r#"class Bar {
    [|{|"parts": ["(","method",")"," ","Bar",".","hello","(",")",":"," ","void"], "kind": "method"|}hello|]() {}
}

new Bar().hel/*reference*/lo;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["reference".to_string()]);
    done();
}
