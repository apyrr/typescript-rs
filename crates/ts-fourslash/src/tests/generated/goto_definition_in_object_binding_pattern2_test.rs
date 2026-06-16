#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_goto_definition_in_object_binding_pattern2() {
    let mut t = TestingT;
    run_test_goto_definition_in_object_binding_pattern2(&mut t);
}

fn run_test_goto_definition_in_object_binding_pattern2(t: &mut TestingT) {
    if should_skip_if_failing("TestGotoDefinitionInObjectBindingPattern2") {
        return;
    }
    let content = r"var p0 = ({a/*1*/a}) => {console.log(aa)};
function f2({ [|a/*a1*/1|], [|b/*b1*/1|] }: { /*a1_dest*/a1: number, /*b1_dest*/b1: number } = { a1: 0, b1: 0 }) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string(), "a1".to_string(), "b1".to_string()]);
    done();
}
