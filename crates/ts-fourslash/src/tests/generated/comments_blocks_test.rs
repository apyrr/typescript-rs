#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_comments_blocks() {
    let mut t = TestingT;
    run_test_comments_blocks(&mut t);
}

fn run_test_comments_blocks(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/// 1
var x,
    /*2*/// 2
    y,
/*3*/     /* %3 */
    z;

/*4*/ // 4
switch (x) {
/*5*/     // 5
    case 1:
/*6*/         // 6
        break;
/*7*/     // 7
    case 2:
/*8*/     // 8
}

/*9*/ // 9
if (true)
/*10*/     // 10
    ;
/*11*/ // 11
else {
/*12*/     // 12
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "// 1");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    // 2");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    /* %3 */");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "// 4");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    // 5");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "        // 6");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "    // 7");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    // 8");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "// 9");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "    // 10");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "// 11");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "    // 12");
    done();
}
