#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_undeclared_property_function_empty_class() {
    let mut t = TestingT;
    run_test_code_fix_undeclared_property_function_empty_class(&mut t);
}

fn run_test_code_fix_undeclared_property_function_empty_class(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
[|class A {
    constructor() {
        this.x = function(x: number, y?: A){
            return x > 0 ? x : y;
        }
    }
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "\nclass A {\n    x: (x: number, y?: A) => A;\n    constructor() {\n        this.x = function(x: number, y?: A){\n            return x > 0 ? x : y;\n        }\n    }\n}\n", false, 0, 0);
    done();
}
