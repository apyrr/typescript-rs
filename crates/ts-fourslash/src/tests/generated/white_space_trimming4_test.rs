#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_white_space_trimming4() {
    let mut t = TestingT;
    run_test_white_space_trimming4(&mut t);
}

fn run_test_white_space_trimming4(t: &mut TestingT) {
    if should_skip_if_failing("TestWhiteSpaceTrimming4") {
        return;
    }
    let content = r"var re = /\w+   /*1*//;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_current_file_content(
        t,
        r"var re = /\w+
    /;",
    );
    done();
}
