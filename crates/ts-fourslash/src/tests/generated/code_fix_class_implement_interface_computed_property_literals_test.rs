#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_computed_property_literals() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_computed_property_literals(&mut t);
}

fn run_test_code_fix_class_implement_interface_computed_property_literals(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface I {
    ["foo"](o: any): boolean;
    ["x"]: boolean;
    [1](): string;
    [2]: boolean;
}

class C implements I {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r#"interface I {
    ["foo"](o: any): boolean;
    ["x"]: boolean;
    [1](): string;
    [2]: boolean;
}

class C implements I {
    ["foo"](o: any): boolean {
        throw new Error("Method not implemented.");
    }
    ["x"]: boolean;
    [1](): string {
        throw new Error("Method not implemented.");
    }
    [2]: boolean;
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
