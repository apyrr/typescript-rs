#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_extend_abstract_some_properties_present() {
    let mut t = TestingT;
    run_test_code_fix_class_extend_abstract_some_properties_present(&mut t);
}

fn run_test_code_fix_class_extend_abstract_some_properties_present(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassExtendAbstractSomePropertiesPresent") {
        return;
    }
    let content = r"// @strict: false
// @noImplicitOverride: true
abstract class A {
   abstract x: number;
   abstract y: number;
   abstract z: number;
}

class C extends A {[|   
   |]constructor(public x: number) { super(); }
   y: number;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "\noverride z: number;\n", false, 0, 0);
    done();
}
