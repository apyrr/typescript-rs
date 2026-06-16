#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_on_definition2() {
    let mut t = TestingT;
    run_test_find_all_refs_on_definition2(&mut t);
}

fn run_test_find_all_refs_on_definition2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"//@Filename: findAllRefsOnDefinition2-import.ts
export module Test{

    /*1*/export interface /*2*/start { }

    export interface stop { }
}
//@Filename: findAllRefsOnDefinition2.ts
import Second = require("./findAllRefsOnDefinition2-import");

var start: Second.Test./*3*/start;
var stop: Second.Test.stop;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
