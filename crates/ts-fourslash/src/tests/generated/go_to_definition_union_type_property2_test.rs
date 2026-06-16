#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_union_type_property2() {
    let mut t = TestingT;
    run_test_go_to_definition_union_type_property2(&mut t);
}

fn run_test_go_to_definition_union_type_property2(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionUnionTypeProperty2") {
        return;
    }
    let content = r"interface HasAOrB {
    /*propertyDefinition1*/a: string;
    b: string;
}

interface One {
    common: { /*propertyDefinition2*/a : number; };
}

interface Two {
    common: HasAOrB;
}

var x : One | Two;

x.common.[|/*propertyReference*/a|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["propertyReference".to_string()]);
    done();
}
