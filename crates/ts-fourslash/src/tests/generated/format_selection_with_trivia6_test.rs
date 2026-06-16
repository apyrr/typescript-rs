#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_selection_with_trivia6() {
    let mut t = TestingT;
    run_test_format_selection_with_trivia6(&mut t);
}

fn run_test_format_selection_with_trivia6(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatSelectionWithTrivia6") {
        return;
    }
    let content = r"/*begin*/    // test comment
/*end*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "begin", "end");
    f.verify_current_file_content(
        t,
        r"// test comment
",
    );
    done();
}
