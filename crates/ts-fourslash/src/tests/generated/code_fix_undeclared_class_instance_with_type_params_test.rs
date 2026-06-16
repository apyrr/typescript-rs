#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_undeclared_class_instance_with_type_params() {
    let mut t = TestingT;
    run_test_code_fix_undeclared_class_instance_with_type_params(&mut t);
}

fn run_test_code_fix_undeclared_class_instance_with_type_params(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
class A<T> {
    a: number;
    b: string;
    constructor(public x: T) {}
}
[|class B {
    constructor() {
        this.x = new A(3);
    }
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "\nclass B {\n    x: A<number>;\n\n    constructor() {\n        this.x = new A(3);\n    }\n}\n", false, 0, 0);
    done();
}
