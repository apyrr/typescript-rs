#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_class_multiple_signatures1() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_class_multiple_signatures1(&mut t);
}

fn run_test_code_fix_class_implement_class_multiple_signatures1(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementClassMultipleSignatures1") {
        return;
    }
    let content = r"class A {
    method(a: number, b: string): boolean;
    method(a: string | number, b?: string | number): boolean | Function { return a + b as any; }
}
class C implements A {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'A'".to_string(),
            new_file_content: r#"class A {
    method(a: number, b: string): boolean;
    method(a: string | number, b?: string | number): boolean | Function { return a + b as any; }
}
class C implements A {
    method(a: number, b: string): boolean;
    method(a: string | number, b?: string | number): boolean | Function {
        throw new Error("Method not implemented.");
    }
}"#
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
