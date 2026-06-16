#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_member() {
    let mut t = TestingT;
    run_test_go_to_definition_member(&mut t);
}

fn run_test_go_to_definition_member(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionMember") {
        return;
    }
    let content = r"// @Filename: /a.ts
class A {
    private z/*z*/: string;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["z".to_string()]);
    done();
}
