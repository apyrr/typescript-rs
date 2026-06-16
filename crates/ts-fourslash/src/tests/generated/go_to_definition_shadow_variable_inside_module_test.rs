#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_shadow_variable_inside_module() {
    let mut t = TestingT;
    run_test_go_to_definition_shadow_variable_inside_module(&mut t);
}

fn run_test_go_to_definition_shadow_variable_inside_module(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionShadowVariableInsideModule") {
        return;
    }
    let content = r"namespace shdModule {
    var /*shadowVariableDefinition*/shdVar;
    /*shadowVariableReference*/shdVar = 1;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["shadowVariableReference".to_string()]);
    done();
}
