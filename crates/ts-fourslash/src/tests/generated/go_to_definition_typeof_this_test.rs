#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_typeof_this() {
    let mut t = TestingT;
    run_test_go_to_definition_typeof_this(&mut t);
}

fn run_test_go_to_definition_typeof_this(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionTypeofThis") {
        return;
    }
    let content = r"function f(/*fnDecl*/this: number) {
    type X = typeof [|/*fnUse*/this|];
}
class /*cls*/C {
    constructor() { type X = typeof [|/*clsUse*/this|]; }
    get self(/*getterDecl*/this: number) { type X = typeof [|/*getterUse*/this|]; }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "fnUse".to_string(),
            "clsUse".to_string(),
            "getterUse".to_string(),
        ],
    );
    done();
}
