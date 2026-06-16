#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_from_link_tag_reference4() {
    let mut t = TestingT;
    run_test_find_all_references_from_link_tag_reference4(&mut t);
}

fn run_test_find_all_references_from_link_tag_reference4(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesFromLinkTagReference4") {
        return;
    }
    let content = r"enum E {
    /** {@link /**/B} */
    A,
    B
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
