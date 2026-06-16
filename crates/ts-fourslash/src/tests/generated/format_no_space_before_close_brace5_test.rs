#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_no_space_before_close_brace5() {
    let mut t = TestingT;
    run_test_format_no_space_before_close_brace5(&mut t);
}

fn run_test_format_no_space_before_close_brace5(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatNoSpaceBeforeCloseBrace5") {
        return;
    }
    let content = r"new Foo(1, 
    /* comment */    );";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"new Foo(1,
    /* comment */);",
    );
    done();
}
