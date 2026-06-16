#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semicolon_formatting_nested_statements() {
    let mut t = TestingT;
    run_test_semicolon_formatting_nested_statements(&mut t);
}

fn run_test_semicolon_formatting_nested_statements(t: &mut TestingT) {
    if should_skip_if_failing("TestSemicolonFormattingNestedStatements") {
        return;
    }
    let content = r"if (true)
if (true)/*parentOutsideBlock*/
if (true) {
if (true)/*directParent*/
var x = 0/*innermost*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "innermost");
    f.insert(t, ";");
    f.verify_current_line_content(t, "        var x = 0;");
    f.go_to_marker(t, "directParent");
    f.verify_current_line_content(t, "    if (true)");
    f.go_to_marker(t, "parentOutsideBlock");
    f.verify_current_line_content(t, "if (true)");
    done();
}
