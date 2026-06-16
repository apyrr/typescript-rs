#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_undeclared_property_numeric_literal() {
    let mut t = TestingT;
    run_test_code_fix_undeclared_property_numeric_literal(&mut t);
}

fn run_test_code_fix_undeclared_property_numeric_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"[|class A {
    constructor() {
        this.x = 10;
    }
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "\nclass A {\n    x: number;\n\n    constructor() {\n        this.x = 10;\n    }\n}\n",
        false,
        0,
        0,
    );
    done();
}
