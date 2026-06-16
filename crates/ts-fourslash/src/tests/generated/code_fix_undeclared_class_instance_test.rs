#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_undeclared_class_instance() {
    let mut t = TestingT;
    run_test_code_fix_undeclared_class_instance(&mut t);
}

fn run_test_code_fix_undeclared_class_instance(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixUndeclaredClassInstance") {
        return;
    }
    let content = r"// @strict: false
class A {
    a: number;
    b: string;
    constructor(public x: any) {}
}
[|class B {
    constructor() {
        this.x = new A(3);
    }
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "\nclass B {\n    x: A;\n\n    constructor() {\n        this.x = new A(3);\n    }\n}\n",
        false,
        0,
        0,
    );
    done();
}
