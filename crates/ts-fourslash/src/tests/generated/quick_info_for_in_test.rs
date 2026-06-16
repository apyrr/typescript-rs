#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_in() {
    let mut t = TestingT;
    run_test_quick_info_for_in(&mut t);
}

fn run_test_quick_info_for_in(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForIn") {
        return;
    }
    let content = r"var obj;
for (var /**/p in obj) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var p: string", "");
    done();
}
