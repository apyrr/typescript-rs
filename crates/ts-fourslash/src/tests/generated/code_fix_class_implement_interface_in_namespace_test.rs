#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_in_namespace() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_in_namespace(&mut t);
}

fn run_test_code_fix_class_implement_interface_in_namespace(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace N1 {
    export interface I1 {
        f1():string;
    }
}
interface I1 {
    f1();
}

class C1 implements N1.I1 {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'N1.I1'".to_string(),
            new_file_content: r#"namespace N1 {
    export interface I1 {
        f1():string;
    }
}
interface I1 {
    f1();
}

class C1 implements N1.I1 {
    f1(): string {
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
