#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_in_destructuring2() {
    let mut t = TestingT;
    run_test_formatting_in_destructuring2(&mut t);
}

fn run_test_formatting_in_destructuring2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"/*1*/function   drawText(    { text = "", location: [x, y]=           [0, 0], bold = false }) {
    // Draw text  
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(
        t,
        "function drawText({ text = \"\", location: [x, y] = [0, 0], bold = false }) {",
    );
    done();
}
