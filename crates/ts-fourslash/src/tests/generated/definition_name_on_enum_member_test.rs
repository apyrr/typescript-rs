#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_definition_name_on_enum_member() {
    let mut t = TestingT;
    run_test_definition_name_on_enum_member(&mut t);
}

fn run_test_definition_name_on_enum_member(t: &mut TestingT) {
    if should_skip_if_failing("TestDefinitionNameOnEnumMember") {
        return;
    }
    let content = r"enum e {
    firstMember,
    secondMember,
    thirdMember
}
var enumMember = e.[|/*1*/thirdMember|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
