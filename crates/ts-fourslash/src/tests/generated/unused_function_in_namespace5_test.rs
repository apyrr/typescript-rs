#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_function_in_namespace5() {
    let mut t = TestingT;
    run_test_unused_function_in_namespace5(&mut t);
}

fn run_test_unused_function_in_namespace5(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedFunctionInNamespace5") {
        return;
    }
    let content = r"// @noUnusedLocals: true
// @noUnusedParameters:true
namespace Validation {
    var function1 = function() {
    }

    export function function2() {

    }

    [| function function3() {
        function1();
    }

    function function4() {

    }

    export let a = function3; |]
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "function function3() {\n        function1();\n    }\n\n    export let a = function3;",
        false,
        0,
        0,
    );
    done();
}
