#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_name_on_enum_member() {
    let mut t = TestingT;
    run_test_rename_name_on_enum_member(&mut t);
}

fn run_test_rename_name_on_enum_member(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameNameOnEnumMember") {
        return;
    }
    let content = r"enum e {
    firstMember,
    secondMember,
    thirdMember
}
var enumMember = e.[|/**/thirdMember|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_rename_succeeded_at_current_position();
    done();
}
