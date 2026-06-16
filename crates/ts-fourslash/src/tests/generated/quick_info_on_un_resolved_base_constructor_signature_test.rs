#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_un_resolved_base_constructor_signature() {
    let mut t = TestingT;
    run_test_quick_info_on_un_resolved_base_constructor_signature(&mut t);
}

fn run_test_quick_info_on_un_resolved_base_constructor_signature(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class baseClassWithConstructorParameterSpecifyingType {
    constructor(loading?: boolean) {
    }
}
class genericBaseClassInheritingConstructorFromBase<TValue> extends baseClassWithConstructorParameterSpecifyingType {
}
class classInheritingSpecializedClass extends genericBaseClassInheritingConstructorFromBase<string> {
}
new class/*1*/InheritingSpecializedClass();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_quick_info_exists(t);
    done();
}
