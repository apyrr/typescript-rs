#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_undeclared_property_object_literal() {
    let mut t = TestingT;
    run_test_code_fix_undeclared_property_object_literal(&mut t);
}

fn run_test_code_fix_undeclared_property_object_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixUndeclaredPropertyObjectLiteral") {
        return;
    }
    let content = r#"// @strict: false
[|class A {
    constructor() {
        let e: any = 10;
        this.x = { a: 10, b: "hello", c: undefined, d: null, e: e };
    }
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "\nclass A {\n    x: { a: number; b: string; c: any; d: any; e: any; };\n    \n    constructor() {\n        let e: any = 10;\n        this.x = { a: 10, b: \"hello\", c: undefined, d: null, e: e };\n    }\n}\n", false, 0, 0);
    done();
}
