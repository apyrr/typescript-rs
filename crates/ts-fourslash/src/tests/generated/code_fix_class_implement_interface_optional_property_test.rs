#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_optional_property() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_optional_property(&mut t);
}

fn run_test_code_fix_class_implement_interface_optional_property(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
interface IPerson {
    name: string;
    birthday?: string;
}
class Person implements IPerson {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'IPerson'".to_string(),
            new_file_content: r"interface IPerson {
    name: string;
    birthday?: string;
}
class Person implements IPerson {
    name: string;
    birthday?: string;
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
