#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_call_property() {
    let mut t = TestingT;
    run_test_quick_info_call_property(&mut t);
}

fn run_test_quick_info_call_property(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoCallProperty") {
        return;
    }
    let content = r"interface I {
    /** Doc */
    m: () => void;
}
function f(x: I): void {
    x./**/m();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(property) I.m: () => void", "Doc");
    done();
}
