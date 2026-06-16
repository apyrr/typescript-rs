#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_regex_detection() {
    let mut t = TestingT;
    run_test_regex_detection(&mut t);
}

fn run_test_regex_detection(t: &mut TestingT) {
    if should_skip_if_failing("TestRegexDetection") {
        return;
    }
    let content = r" /*1*/15 / /*2*/Math.min(61 / /*3*/42, 32 / 15) / /*4*/15;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_not_quick_info_exists(t);
    f.go_to_marker(t, "2");
    f.verify_quick_info_is(
        t,
        "var Math: Math",
        "An intrinsic object that provides basic mathematics functionality and constants.",
    );
    f.go_to_marker(t, "3");
    f.verify_not_quick_info_exists(t);
    f.go_to_marker(t, "4");
    f.verify_not_quick_info_exists(t);
    done();
}
