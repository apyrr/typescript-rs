#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_completion_on_right_side_of_import() {
    let mut t = TestingT;
    run_test_member_completion_on_right_side_of_import(&mut t);
}

fn run_test_member_completion_on_right_side_of_import(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberCompletionOnRightSideOfImport") {
        return;
    }
    let content = r"import x = M./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}
