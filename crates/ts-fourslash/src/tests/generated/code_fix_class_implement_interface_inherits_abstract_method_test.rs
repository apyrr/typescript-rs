#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_inherits_abstract_method() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_inherits_abstract_method(&mut t);
}

fn run_test_code_fix_class_implement_interface_inherits_abstract_method(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"abstract class C1 { }
abstract class C2 {
    abstract fＡ<T extends number>(): T;
}
interface I1 extends C1, C2 { }
class C3 implements I1 {[| |]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I1'".to_string(),
            new_file_content: r#"abstract class C1 { }
abstract class C2 {
    abstract fＡ<T extends number>(): T;
}
interface I1 extends C1, C2 { }
class C3 implements I1 {
    fＡ<T extends number>(): T {
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
