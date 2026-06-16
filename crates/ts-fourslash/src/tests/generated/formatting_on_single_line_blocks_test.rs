#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_single_line_blocks() {
    let mut t = TestingT;
    run_test_formatting_on_single_line_blocks(&mut t);
}

fn run_test_formatting_on_single_line_blocks(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnSingleLineBlocks") {
        return;
    }
    let content = r"class C
{}
if (true)
{}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"class C { }
if (true) { }",
    );
    done();
}
