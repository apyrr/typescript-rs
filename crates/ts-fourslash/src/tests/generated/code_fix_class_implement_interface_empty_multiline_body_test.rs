#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_empty_multiline_body() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_empty_multiline_body(&mut t);
}

fn run_test_code_fix_class_implement_interface_empty_multiline_body(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es2017
interface I {
    x: number;
    y: number;
}
class C implements I {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r"interface I {
    x: number;
    y: number;
}
class C implements I {
    x: number;
    y: number;
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
