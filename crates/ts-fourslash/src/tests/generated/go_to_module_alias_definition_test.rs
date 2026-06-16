#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_module_alias_definition() {
    let mut t = TestingT;
    run_test_go_to_module_alias_definition(&mut t);
}

fn run_test_go_to_module_alias_definition(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToModuleAliasDefinition") {
        return;
    }
    let content = r"// @Filename: a.ts
export class /*2*/Foo {}
// @Filename: b.ts
 import /*3*/n = require('a');
 var x = new [|/*1*/n|].Foo();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
