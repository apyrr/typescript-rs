#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_not_inside_comment() {
    let mut t = TestingT;
    run_test_quick_info_not_inside_comment(&mut t);
}

fn run_test_quick_info_not_inside_comment(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfo_notInsideComment") {
        return;
    }
    let content = r"a/* /**/ */.b";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_not_quick_info_exists(t);
    done();
}
