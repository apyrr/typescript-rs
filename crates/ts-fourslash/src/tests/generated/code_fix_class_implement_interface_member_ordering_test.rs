#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_member_ordering() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_member_ordering(&mut t);
}

fn run_test_code_fix_class_implement_interface_member_ordering(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es2017
/** asdf */
interface I {
    1;
    2;
    3;
    4;
    5;
    6;
    7;
    8;
    9;
    10;
    11;
    12;
    13;
    14;
    15;
    16;
    17;
    18;
    19;
    20;
    21;
    22;
    /** a nice safe prime */
    23;
}
class C implements I {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r"/** asdf */
interface I {
    1;
    2;
    3;
    4;
    5;
    6;
    7;
    8;
    9;
    10;
    11;
    12;
    13;
    14;
    15;
    16;
    17;
    18;
    19;
    20;
    21;
    22;
    /** a nice safe prime */
    23;
}
class C implements I {
    1: any;
    2: any;
    3: any;
    4: any;
    5: any;
    6: any;
    7: any;
    8: any;
    9: any;
    10: any;
    11: any;
    12: any;
    13: any;
    14: any;
    15: any;
    16: any;
    17: any;
    18: any;
    19: any;
    20: any;
    21: any;
    22: any;
    23: any;
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
