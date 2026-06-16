#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_no_undefined_on_optional_parameter() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_no_undefined_on_optional_parameter(&mut t);
}

fn run_test_code_fix_class_implement_interface_no_undefined_on_optional_parameter(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"interface IFoo {
    bar(x?: number | string): void;
}

class Foo implements IFoo {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'IFoo'".to_string(),
            new_file_content: r#"interface IFoo {
    bar(x?: number | string): void;
}

class Foo implements IFoo {
    bar(x?: number | string): void {
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
