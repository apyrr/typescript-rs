#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_object_binding_element_property_name01() {
    let mut t = TestingT;
    run_test_go_to_definition_object_binding_element_property_name01(&mut t);
}

fn run_test_go_to_definition_object_binding_element_property_name01(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionObjectBindingElementPropertyName01") {
        return;
    }
    let content = r"interface I {
    /*def*/property1: number;
    property2: string;
}

var foo: I;
var { [|/*use*/property1|]: prop1 } = foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
