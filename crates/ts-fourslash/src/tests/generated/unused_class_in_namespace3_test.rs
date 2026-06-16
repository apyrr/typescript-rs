#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_class_in_namespace3() {
    let mut t = TestingT;
    run_test_unused_class_in_namespace3(&mut t);
}

fn run_test_unused_class_in_namespace3(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedClassInNamespace3") {
        return;
    }
    let content = r"// @noUnusedLocals: true
// @noUnusedParameters:true
 [| namespace Validation {
    class c1 {

    }

    export class c2 {

    }

    class c3 extends c1 {

    }
} |]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "namespace Validation {\n    class c1 {\n    }\n\n    export class c2 {\n    }\n}",
        false,
        0,
        0,
    );
    done();
}
