#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_ambiants() {
    let mut t = TestingT;
    run_test_go_to_definition_ambiants(&mut t);
}

fn run_test_go_to_definition_ambiants(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare var /*ambientVariableDefinition*/ambientVar;
declare function /*ambientFunctionDefinition*/ambientFunction();
declare class ambientClass {
    /*constructorDefinition*/constructor();
    static /*staticMethodDefinition*/method();
    public /*instanceMethodDefinition*/method();
}

/*ambientVariableReference*/ambientVar = 1;
/*ambientFunctionReference*/ambientFunction();
var ambientClassVariable = new /*constructorReference*/ambientClass();
ambientClass./*staticMethodReference*/method();
ambientClassVariable./*instanceMethodReference*/method();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "ambientVariableReference".to_string(),
            "ambientFunctionReference".to_string(),
            "constructorReference".to_string(),
            "staticMethodReference".to_string(),
            "instanceMethodReference".to_string(),
        ],
    );
    done();
}
