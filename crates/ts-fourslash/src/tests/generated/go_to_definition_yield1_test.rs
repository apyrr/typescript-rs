#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_yield1() {
    let mut t = TestingT;
    run_test_go_to_definition_yield1(&mut t);
}

fn run_test_go_to_definition_yield1(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionYield1") {
        return;
    }
    let content = r"function* /*end1*/gen() {
    [|/*start1*/yield|] 0;
}

const /*end2*/genFunction = function*() {
    [|/*start2*/yield|] 0;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["start1".to_string(), "start2".to_string()]);
    done();
}
