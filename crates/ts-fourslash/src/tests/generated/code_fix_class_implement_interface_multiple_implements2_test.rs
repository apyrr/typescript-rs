#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_multiple_implements2() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_multiple_implements2(&mut t);
}

fn run_test_code_fix_class_implement_interface_multiple_implements2(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceMultipleImplements2") {
        return;
    }
    let content = r#"// @strict: false
interface I1 {
    x: number;
}
interface I2 {
    y: "𣋝ઢȴ¬⏊";
}

class C implements I1,I2 {[|
    |]x: number;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "\ny: \"𣋝ઢȴ¬⏊\";\n", false, 0, 0);
    f.verify_code_fix_not_available(t, &[]);
    done();
}
