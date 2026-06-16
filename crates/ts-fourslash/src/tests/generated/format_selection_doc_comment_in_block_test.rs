#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_selection_doc_comment_in_block() {
    let mut t = TestingT;
    run_test_format_selection_doc_comment_in_block(&mut t);
}

fn run_test_format_selection_doc_comment_in_block(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatSelectionDocCommentInBlock") {
        return;
    }
    let content = r"{
    /*1*//**
     * Some doc comment
     *//*2*/
    const a = 1;
}

while (true) {
/*3*//**
 * Some doc comment
 *//*4*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "1", "2");
    f.verify_current_file_content(
        t,
        r"{
    /**
     * Some doc comment
     */
    const a = 1;
}

while (true) {
/**
 * Some doc comment
 */
}",
    );
    f.format_selection(t, "3", "4");
    f.verify_current_file_content(
        t,
        r"{
    /**
     * Some doc comment
     */
    const a = 1;
}

while (true) {
    /**
     * Some doc comment
     */
}",
    );
    done();
}
