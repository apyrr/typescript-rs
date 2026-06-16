#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_multiple_members_and_punctuation() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_multiple_members_and_punctuation(&mut t);
}

fn run_test_code_fix_class_implement_interface_multiple_members_and_punctuation(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I1 {
    x: number,
    y: number
    z: number;
    f(): number,
    g(): any
    h();
}

class C1 implements I1 {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I1'".to_string(),
            new_file_content: r#"interface I1 {
    x: number,
    y: number
    z: number;
    f(): number,
    g(): any
    h();
}

class C1 implements I1 {
    x: number;
    y: number;
    z: number;
    f(): number {
        throw new Error("Method not implemented.");
    }
    g() {
        throw new Error("Method not implemented.");
    }
    h() {
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
