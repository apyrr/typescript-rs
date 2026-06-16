#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_new_expression_target_not_class() {
    let mut t = TestingT;
    run_test_go_to_definition_new_expression_target_not_class(&mut t);
}

fn run_test_go_to_definition_new_expression_target_not_class(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionNewExpressionTargetNotClass") {
        return;
    }
    let content = r"class C2 {
}
let /*I*/I: {
    /*constructSignature*/new(): C2;
};
new [|/*invokeExpression1*/I|]();
let /*symbolDeclaration*/I2: {
};
new [|/*invokeExpression2*/I2|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "invokeExpression1".to_string(),
            "invokeExpression2".to_string(),
        ],
    );
    done();
}
