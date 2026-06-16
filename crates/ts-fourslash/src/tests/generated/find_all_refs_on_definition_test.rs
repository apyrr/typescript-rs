#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_on_definition() {
    let mut t = TestingT;
    run_test_find_all_refs_on_definition(&mut t);
}

fn run_test_find_all_refs_on_definition(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsOnDefinition") {
        return;
    }
    let content = r#"//@Filename: findAllRefsOnDefinition-import.ts
export class Test{

    constructor(){

    }

    /*1*/public /*2*/start(){
        return this;
    }

    public stop(){
        return this;
    }
}
//@Filename: findAllRefsOnDefinition.ts
import Second = require("./findAllRefsOnDefinition-import");

var second = new Second.Test()
second./*3*/start();
second.stop();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
