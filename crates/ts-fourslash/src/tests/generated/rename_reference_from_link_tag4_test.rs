#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_reference_from_link_tag4() {
    let mut t = TestingT;
    run_test_rename_reference_from_link_tag4(&mut t);
}

fn run_test_rename_reference_from_link_tag4(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameReferenceFromLinkTag4") {
        return;
    }
    let content = r"enum E {
    /** {@link /**/B} */
    A,
    B
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
