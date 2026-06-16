#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_multiline_comments_with_tabs1() {
    let mut t = TestingT;
    run_test_formatting_multiline_comments_with_tabs1(&mut t);
}

fn run_test_formatting_multiline_comments_with_tabs1(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingMultilineCommentsWithTabs1") {
        return;
    }
    let content = r"var f = function (j) {

	switch (j) {
		case 1:
/*1*/				/* when current checkbox has focus, Firefox has changed check state already
/*2*/				on SPACE bar press only
/*3*/				IE does not have issue, use the CSS class
/*4*/				input:focus[type=checkbox] (z-index = 31290)
/*5*/				to determine whether checkbox has focus or not
				*/
			break;
		case 2:
		break;
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(
        t,
        "            /* when current checkbox has focus, Firefox has changed check state already",
    );
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "            on SPACE bar press only");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "            IE does not have issue, use the CSS class");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(
        t,
        "            input:focus[type=checkbox] (z-index = 31290)",
    );
    f.go_to_marker(t, "5");
    f.verify_current_line_content(
        t,
        "            to determine whether checkbox has focus or not",
    );
    done();
}
