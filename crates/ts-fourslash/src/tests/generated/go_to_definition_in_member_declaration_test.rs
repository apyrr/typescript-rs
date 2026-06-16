#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_in_member_declaration() {
    let mut t = TestingT;
    run_test_go_to_definition_in_member_declaration(&mut t);
}

fn run_test_go_to_definition_in_member_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface /*interfaceDefinition*/IFoo { method1(): number; }

class /*classDefinition*/Foo implements IFoo {
    public method1(): number { return 0; }
}

enum /*enumDefinition*/Enum { value1, value2 };

class /*selfDefinition*/Bar {
    public _interface: [|IFo/*interfaceReference*/o|] = new [|Fo/*classReferenceInInitializer*/o|]();
    public _class: [|Fo/*classReference*/o|] = new Foo();
    public _list: [|IF/*interfaceReferenceInList*/oo|][]=[];
    public _enum: [|E/*enumReference*/num|] = [|En/*enumReferenceInInitializer*/um|].value1;
    public _self: [|Ba/*selfReference*/r|];

    constructor(public _inConstructor: [|IFo/*interfaceReferenceInConstructor*/o|]) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "interfaceReference".to_string(),
            "interfaceReferenceInList".to_string(),
            "interfaceReferenceInConstructor".to_string(),
            "classReference".to_string(),
            "classReferenceInInitializer".to_string(),
            "enumReference".to_string(),
            "enumReferenceInInitializer".to_string(),
            "selfReference".to_string(),
        ],
    );
    done();
}
