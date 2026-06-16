#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_error_types1() {
    let mut t = TestingT;
    run_test_quick_info_on_error_types1(&mut t);
}

fn run_test_quick_info_on_error_types1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnErrorTypes1") {
        return;
    }
    let content = r"var /*A*/f: {
    x: number;
    <
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "A", "var f: {\n    (): any;\n    x: number;\n}", "");
    done();
}
