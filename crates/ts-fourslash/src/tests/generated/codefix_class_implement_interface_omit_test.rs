#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_codefix_class_implement_interface_omit() {
    let mut t = TestingT;
    run_test_codefix_class_implement_interface_omit(&mut t);
}

fn run_test_codefix_class_implement_interface_omit(t: &mut TestingT) {
    if should_skip_if_failing("TestCodefixClassImplementInterface_omit") {
        return;
    }
    let content = r#"interface One {
    a: number;
    b: string;
}

interface Two extends Omit<One, "a"> {
    c: boolean;
}

class TwoStore implements Two {[| |]}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Two'".to_string(),
            new_file_content: r#"interface One {
    a: number;
    b: string;
}

interface Two extends Omit<One, "a"> {
    c: boolean;
}

class TwoStore implements Two {
    c: boolean;
    b: string;
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
