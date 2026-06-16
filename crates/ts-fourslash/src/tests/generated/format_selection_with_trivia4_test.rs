#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_selection_with_trivia4() {
    let mut t = TestingT;
    run_test_format_selection_with_trivia4(&mut t);
}

fn run_test_format_selection_with_trivia4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"if (true) {
/*begin*/// test comment
/*end*/console.log();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "begin", "end");
    f.verify_current_file_content(
        t,
        r"if (true) {
    // test comment
console.log();
}",
    );
    done();
}
