#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_default_class() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_default_class(&mut t);
}

fn run_test_code_fix_class_implement_default_class(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementDefaultClass") {
        return;
    }
    let content = r"interface I { x: number; }
export default class implements I {[| |]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r"interface I { x: number; }
export default class implements I {
    x: number;
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
