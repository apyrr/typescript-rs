#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_crash() {
    let mut t = TestingT;
    run_test_formatting_crash(&mut t);
}

fn run_test_formatting_crash(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingCrash") {
        return;
    }
    let content = r"/**/module Default{ 
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_functions = ts_core::TSTrue;
        f.configure(t, opts);
    }
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_control_blocks = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "");
    f.verify_current_line_content(t, "module Default");
    done();
}
