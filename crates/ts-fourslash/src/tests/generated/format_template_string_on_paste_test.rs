#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_template_string_on_paste() {
    let mut t = TestingT;
    run_test_format_template_string_on_paste(&mut t);
}

fn run_test_format_template_string_on_paste(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatTemplateStringOnPaste") {
        return;
    }
    let content = r"const x = `${0}/*0*/abc/*1*/`;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "0", "1");
    f.verify_current_file_content(t, r"const x = `${0}abc`;");
    done();
}
