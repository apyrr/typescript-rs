#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_insert_space_after_close_brace_before_close_bracket() {
    let mut t = TestingT;
    run_test_format_insert_space_after_close_brace_before_close_bracket(&mut t);
}

fn run_test_format_insert_space_after_close_brace_before_close_bracket(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatInsertSpaceAfterCloseBraceBeforeCloseBracket") {
        return;
    }
    let content = r"[{}]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_opening_and_before_closing_nonempty_brackets = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(t, r"[ {} ]");
    done();
}
