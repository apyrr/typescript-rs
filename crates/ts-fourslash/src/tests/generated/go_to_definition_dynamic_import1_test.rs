#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_dynamic_import1() {
    let mut t = TestingT;
    run_test_go_to_definition_dynamic_import1(&mut t);
}

fn run_test_go_to_definition_dynamic_import1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: foo.ts
/*Destination*/export function foo() { return "foo"; }
import([|"./f/*1*/oo"|])
var x = import([|"./fo/*2*/o"|])"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string(), "2".to_string()]);
    done();
}
