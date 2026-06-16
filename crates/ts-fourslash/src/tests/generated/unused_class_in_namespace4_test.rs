#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_class_in_namespace4() {
    let mut t = TestingT;
    run_test_unused_class_in_namespace4(&mut t);
}

fn run_test_unused_class_in_namespace4(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedClassInNamespace4") {
        return;
    }
    let content = r"// @strict: false
// @noUnusedLocals: true
// @noUnusedParameters:true
 [| namespace Validation {
    class c1 {

    }

    export class c2 {

    }

    class c3 {
        public x: c1;
    }
} |]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "namespace Validation {\n    class c1 {\n\n    }\n\n    export class c2 {\n\n    }\n}",
        false,
        0,
        0,
    );
    done();
}
