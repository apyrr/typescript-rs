#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_object_literal_properties() {
    let mut t = TestingT;
    run_test_go_to_definition_object_literal_properties(&mut t);
}

fn run_test_go_to_definition_object_literal_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionObjectLiteralProperties") {
        return;
    }
    let content = r"var o = {
    /*valueDefinition*/value: 0,
    get /*getterDefinition*/getter() {return 0 },
    set /*setterDefinition*/setter(v: number) { },
    /*methodDefinition*/method: () => { },
    /*es6StyleMethodDefinition*/es6StyleMethod() { }
};

o./*valueReference*/value;
o./*getterReference*/getter;
o./*setterReference*/setter;
o./*methodReference*/method;
o./*es6StyleMethodReference*/es6StyleMethod;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "valueReference".to_string(),
            "getterReference".to_string(),
            "setterReference".to_string(),
            "methodReference".to_string(),
            "es6StyleMethodReference".to_string(),
        ],
    );
    done();
}
