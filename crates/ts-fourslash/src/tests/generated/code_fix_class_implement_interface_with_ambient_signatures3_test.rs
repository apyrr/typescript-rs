#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_with_ambient_signatures3() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_with_ambient_signatures3(&mut t);
}

fn run_test_code_fix_class_implement_interface_with_ambient_signatures3(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceWithAmbientSignatures3") {
        return;
    }
    let content = r"declare abstract class A {
    abstract method(): void;
}
class B implements A {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'A'".to_string(),
            new_file_content: r#"declare abstract class A {
    abstract method(): void;
}
class B implements A {
    method(): void {
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
