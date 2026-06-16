#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_all() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_all(&mut t);
}

fn run_test_code_fix_class_implement_interface_all(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterface_all") {
        return;
    }
    let content = r"interface I { i(): void; }
interface J { j(): void; }
class C implements I, J {}
class D implements J {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixClassIncorrectlyImplementsInterface".to_string(),
            new_file_content: r#"interface I { i(): void; }
interface J { j(): void; }
class C implements I, J {
    i(): void {
        throw new Error("Method not implemented.");
    }
    j(): void {
        throw new Error("Method not implemented.");
    }
}
class D implements J {
    j(): void {
        throw new Error("Method not implemented.");
    }
}"#
            .to_string(),
        },
    );
    done();
}
