#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_simple() {
    let mut t = TestingT;
    run_test_go_to_definition_simple(&mut t);
}

fn run_test_go_to_definition_simple(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: Definition.ts
class /*2*/c { }
// @Filename: Consumption.ts
 var n = new [|/*1*/c|]();
 var n = new [|c/*3*/|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string(), "3".to_string()]);
    done();
}
