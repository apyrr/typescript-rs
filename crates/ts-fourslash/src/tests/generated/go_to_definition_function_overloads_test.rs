#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_function_overloads() {
    let mut t = TestingT;
    run_test_go_to_definition_function_overloads(&mut t);
}

fn run_test_go_to_definition_function_overloads(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionFunctionOverloads") {
        return;
    }
    let content = r#"function [|/*functionOverload1*/functionOverload|](value: number);
function /*functionOverload2*/functionOverload(value: string);
function /*functionOverloadDefinition*/functionOverload() {}

[|/*functionOverloadReference1*/functionOverload|](123);
[|/*functionOverloadReference2*/functionOverload|]("123");
[|/*brokenOverload*/functionOverload|]({});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "functionOverloadReference1".to_string(),
            "functionOverloadReference2".to_string(),
            "brokenOverload".to_string(),
            "functionOverload1".to_string(),
        ],
    );
    done();
}
