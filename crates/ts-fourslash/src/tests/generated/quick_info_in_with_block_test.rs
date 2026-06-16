#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_in_with_block() {
    let mut t = TestingT;
    run_test_quick_info_in_with_block(&mut t);
}

fn run_test_quick_info_in_with_block(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInWithBlock") {
        return;
    }
    let content = r"with (x) {
    function /*1*/f() { }
    var /*2*/b = /*3*/f;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "any", "");
    f.verify_quick_info_at(t, "2", "any", "");
    f.verify_quick_info_at(t, "3", "any", "");
    done();
}
