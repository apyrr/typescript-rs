#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_after_single_dot() {
    let mut t = TestingT;
    run_test_member_list_after_single_dot(&mut t);
}

fn run_test_member_list_after_single_dot(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberListAfterSingleDot") {
        return;
    }
    let content = r"./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}
