#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_shorthand_property01() {
    let mut t = TestingT;
    run_test_go_to_definition_shorthand_property01(&mut t);
}

fn run_test_go_to_definition_shorthand_property01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
var /*valueDeclaration1*/name = "hello";
var /*valueDeclaration2*/id = 100000;
declare var /*valueDeclaration3*/id;
var obj = {[|/*valueDefinition1*/name|], [|/*valueDefinition2*/id|]};
obj.[|/*valueReference1*/name|];
obj.[|/*valueReference2*/id|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "valueDefinition1".to_string(),
            "valueDefinition2".to_string(),
            "valueReference1".to_string(),
            "valueReference2".to_string(),
        ],
    );
    done();
}
