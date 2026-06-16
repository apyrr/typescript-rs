#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_parameter() {
    let mut t = TestingT;
    run_test_format_parameter(&mut t);
}

fn run_test_format_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatParameter") {
        return;
    }
    let content = r"function foo(
    first:
    number,/*first*/
    second: (
    string/*second*/
    ),
    third:
    (
    boolean/*third*/
    )
) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "first");
    f.verify_current_line_content(t, "        number,");
    f.go_to_marker(t, "second");
    f.verify_current_line_content(t, "        string");
    f.go_to_marker(t, "third");
    f.verify_current_line_content(t, "            boolean");
    done();
}
