#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_goto_definition_property_access_expression_heritage_clause() {
    let mut t = TestingT;
    run_test_goto_definition_property_access_expression_heritage_clause(&mut t);
}

fn run_test_goto_definition_property_access_expression_heritage_clause(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class B {}
function foo() {
    return {/*refB*/B: B};
}
class C extends (foo()).[|/*B*/B|] {}
class C1 extends foo().[|/*B1*/B|] {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["B".to_string(), "B1".to_string()]);
    done();
}
