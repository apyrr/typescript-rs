#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_as_operator_formatting() {
    let mut t = TestingT;
    run_test_as_operator_formatting(&mut t);
}

fn run_test_as_operator_formatting(t: &mut TestingT) {
    if should_skip_if_failing("TestAsOperatorFormatting") {
        return;
    }
    let content = r"/**/var x = 3   as  number;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.format_document(t, "");
    f.verify_current_line_content(t, "var x = 3 as number;");
    done();
}
