#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_external_module_name2() {
    let mut t = TestingT;
    run_test_go_to_definition_external_module_name2(&mut t);
}

fn run_test_go_to_definition_external_module_name2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: b.ts
import n = require([|'./a/*1*/'|]);
var x = new n.Foo();
// @Filename: a.ts
/*2*/class Foo {}
export var x = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
