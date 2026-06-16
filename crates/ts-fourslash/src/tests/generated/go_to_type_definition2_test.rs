#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition2() {
    let mut t = TestingT;
    run_test_go_to_type_definition2(&mut t);
}

fn run_test_go_to_type_definition2(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToTypeDefinition2") {
        return;
    }
    let content = r"// @Filename: goToTypeDefinition2_Definition.ts
interface /*definition*/I1 {
    p;
}
type propertyType = I1;
interface I2 {
    property: propertyType;
}
// @Filename: goToTypeDefinition2_Consumption.ts
var i2: I2;
i2.prop/*reference*/erty;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(t, &["reference".to_string()]);
    done();
}
