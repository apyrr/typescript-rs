#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_class_in_namespace2() {
    let mut t = TestingT;
    run_test_unused_class_in_namespace2(&mut t);
}

fn run_test_unused_class_in_namespace2(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedClassInNamespace2") {
        return;
    }
    let content = r"// @noUnusedLocals: true
[| namespace greeter {
   export class class2 {
   }
   class class1 {
   }
} |]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "namespace greeter {\n    export class class2 {\n    }\n}",
        false,
        0,
        0,
    );
    done();
}
