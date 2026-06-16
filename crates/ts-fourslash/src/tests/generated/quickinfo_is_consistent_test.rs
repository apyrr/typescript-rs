#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_is_consistent() {
    let mut t = TestingT;
    run_test_quickinfo_is_consistent(&mut t);
}

fn run_test_quickinfo_is_consistent(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoIsConsistent") {
        return;
    }
    let content = r"declare var /*1*/f: (x: number) => number;
function baz() {
    var x = /*2*/f(3);
    /*3*/f(3);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var f: (x: number) => number", "");
    f.verify_quick_info_at(t, "2", "var f: (x: number) => number", "");
    f.verify_quick_info_at(t, "3", "var f: (x: number) => number", "");
    done();
}
