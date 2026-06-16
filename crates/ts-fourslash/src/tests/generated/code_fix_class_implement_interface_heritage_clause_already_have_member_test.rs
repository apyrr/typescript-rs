#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_heritage_clause_already_have_member() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_heritage_clause_already_have_member(&mut t);
}

fn run_test_code_fix_class_implement_interface_heritage_clause_already_have_member(
    t: &mut TestingT,
) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceHeritageClauseAlreadyHaveMember") {
        return;
    }
    let content = r"// @strict: false
class Base {
    foo: number;
}

class D extends Base {
    bar: number;
}

interface I {
    foo: number;
    bar: number;
    baz: number;
}

class C extends D implements I { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r"class Base {
    foo: number;
}

class D extends Base {
    bar: number;
}

interface I {
    foo: number;
    bar: number;
    baz: number;
}

class C extends D implements I {
    baz: number;
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
