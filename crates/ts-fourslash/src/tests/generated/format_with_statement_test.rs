#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_with_statement() {
    let mut t = TestingT;
    run_test_format_with_statement(&mut t);
}

fn run_test_format_with_statement(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatWithStatement") {
        return;
    }
    let content = r"with /*1*/(foo.bar)

   {/*2*/

     }/*3*/

with (bar.blah)/*4*/
{/*5*/
}/*6*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_control_blocks = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "with (foo.bar) {");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "with (bar.blah) {");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "}");
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_control_blocks = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "with (foo.bar)");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "{");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "with (bar.blah)");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "{");
    done();
}
