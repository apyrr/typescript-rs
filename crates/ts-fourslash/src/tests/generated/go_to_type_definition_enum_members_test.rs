#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_enum_members() {
    let mut t = TestingT;
    run_test_go_to_type_definition_enum_members(&mut t);
}

fn run_test_go_to_type_definition_enum_members(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToTypeDefinitionEnumMembers") {
        return;
    }
    let content = r"enum E {
    value1,
    /*definition*/value2
}
var x = E.value2;

/*reference*/x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(t, &["reference".to_string()]);
    done();
}
