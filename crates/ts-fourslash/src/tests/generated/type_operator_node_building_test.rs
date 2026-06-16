#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_operator_node_building() {
    let mut t = TestingT;
    run_test_type_operator_node_building(&mut t);
}

fn run_test_type_operator_node_building(t: &mut TestingT) {
    if should_skip_if_failing("TestTypeOperatorNodeBuilding") {
        return;
    }
    let content = r"// @Filename: keyof.ts
function doSomethingWithKeys<T>(...keys: (keyof T)[]) { }

const /*1*/utilityFunctions = {
  doSomethingWithKeys
};
// @Filename: typeof.ts
class Foo { static a: number; }
function doSomethingWithTypes(...statics: (typeof Foo)[]) {}

const /*2*/utilityFunctions = {
  doSomethingWithTypes
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "const utilityFunctions: {\n    doSomethingWithKeys: <T>(...keys: (keyof T)[]) => void;\n}",
        "",
    );
    f.verify_quick_info_at(t, "2", "const utilityFunctions: {\n    doSomethingWithTypes: (...statics: (typeof Foo)[]) => void;\n}", "");
    done();
}
