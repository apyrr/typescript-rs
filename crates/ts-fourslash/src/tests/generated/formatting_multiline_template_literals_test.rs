#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_multiline_template_literals() {
    let mut t = TestingT;
    run_test_formatting_multiline_template_literals(&mut t);
}

fn run_test_formatting_multiline_template_literals(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingMultilineTemplateLiterals") {
        return;
    }
    let content = r"/*1*/new Error(`Failed to expand glob: ${projectSpec.filesGlob}
/*2*/                at projectPath : ${projectFile}
/*3*/                with error: ${ex.message}`)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(
        t,
        "new Error(`Failed to expand glob: ${projectSpec.filesGlob}",
    );
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "                at projectPath : ${projectFile}");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "                with error: ${ex.message}`)");
    done();
}
