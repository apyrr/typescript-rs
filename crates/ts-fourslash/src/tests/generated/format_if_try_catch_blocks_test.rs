#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_if_try_catch_blocks() {
    let mut t = TestingT;
    run_test_format_if_try_catch_blocks(&mut t);
}

fn run_test_format_if_try_catch_blocks(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"try {
}
catch {
}

try {
}
catch (e) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_control_blocks = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"try
{
}
catch
{
}

try
{
}
catch (e)
{
}",
    );
    done();
}
