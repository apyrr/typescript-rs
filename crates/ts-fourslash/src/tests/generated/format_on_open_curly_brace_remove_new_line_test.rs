#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_on_open_curly_brace_remove_new_line() {
    let mut t = TestingT;
    run_test_format_on_open_curly_brace_remove_new_line(&mut t);
}

fn run_test_format_on_open_curly_brace_remove_new_line(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatOnOpenCurlyBraceRemoveNewLine") {
        return;
    }
    let content = r"if(true)
/**/ }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_control_blocks = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.go_to_marker(t, "");
    f.insert(t, "{");
    f.verify_current_file_content(t, r"if (true) { }");
    done();
}
