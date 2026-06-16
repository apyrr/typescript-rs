#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_module() {
    let mut t = TestingT;
    run_test_go_to_type_definition_module(&mut t);
}

fn run_test_go_to_type_definition_module(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToTypeDefinitionModule") {
        return;
    }
    let content = r"// @Filename: module1.ts
module /*definition*/M {
    export var p;
}
var m: typeof M;
// @Filename: module3.ts
/*reference1*/M;
/*reference2*/m;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(
        t,
        &["reference1".to_string(), "reference2".to_string()],
    );
    done();
}
