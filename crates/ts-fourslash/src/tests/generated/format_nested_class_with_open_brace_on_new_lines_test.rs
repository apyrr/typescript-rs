#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_nested_class_with_open_brace_on_new_lines() {
    let mut t = TestingT;
    run_test_format_nested_class_with_open_brace_on_new_lines(&mut t);
}

fn run_test_format_nested_class_with_open_brace_on_new_lines(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatNestedClassWithOpenBraceOnNewLines") {
        return;
    }
    let content = r"module A
{
    class B {
        /*1*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_control_blocks = ts_core::TSTrue;
        f.configure(t, opts);
    }
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_functions = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.go_to_marker(t, "1");
    f.insert(t, "}");
    f.verify_current_file_content(
        t,
        r"module A
{
    class B
    {
    }
}",
    );
    done();
}
