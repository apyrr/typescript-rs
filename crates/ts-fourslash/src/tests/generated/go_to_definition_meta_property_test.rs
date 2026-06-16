#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_meta_property() {
    let mut t = TestingT;
    run_test_go_to_definition_meta_property(&mut t);
}

fn run_test_go_to_definition_meta_property(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionMetaProperty") {
        return;
    }
    let content = r"// @Filename: /a.ts
im/*1*/port.met/*2*/a;
function /*functionDefinition*/f() { n/*3*/ew.[|t/*4*/arget|]; }
// @Filename: /b.ts
im/*5*/port.m;
class /*classDefinition*/c { constructor() { n/*6*/ew.[|t/*7*/arget|]; } }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
        ],
    );
    done();
}
