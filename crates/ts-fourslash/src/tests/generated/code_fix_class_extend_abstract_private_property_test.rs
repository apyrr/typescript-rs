#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_extend_abstract_private_property() {
    let mut t = TestingT;
    run_test_code_fix_class_extend_abstract_private_property(&mut t);
}

fn run_test_code_fix_class_extend_abstract_private_property(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassExtendAbstractPrivateProperty") {
        return;
    }
    let content = r"// @noImplicitOverride: true
abstract class A {
   private abstract x: number;
   m() { this.x; } // Avoid unused private
}

class C extends A {[| |]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
