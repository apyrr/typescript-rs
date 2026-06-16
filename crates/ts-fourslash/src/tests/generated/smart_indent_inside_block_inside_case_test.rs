#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_inside_block_inside_case() {
    let mut t = TestingT;
    run_test_smart_indent_inside_block_inside_case(&mut t);
}

fn run_test_smart_indent_inside_block_inside_case(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace SwitchTest {
    var a = 3;

    if (a == 5) {
        switch (a) {
            case 1:
                if (a == 5) {
                    /**/
                }
                break;
        }
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_indentation(t, 20);
    done();
}
