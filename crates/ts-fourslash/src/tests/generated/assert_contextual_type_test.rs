#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_assert_contextual_type() {
    let mut t = TestingT;
    run_test_assert_contextual_type(&mut t);
}

fn run_test_assert_contextual_type(t: &mut TestingT) {
    if should_skip_if_failing("TestAssertContextualType") {
        return;
    }
    let content = r"<(aa: number) =>void >(function myFn(b/**/b) { });";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(parameter) bb: number", "");
    done();
}
