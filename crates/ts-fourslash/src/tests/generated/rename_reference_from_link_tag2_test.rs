#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_reference_from_link_tag2() {
    let mut t = TestingT;
    run_test_rename_reference_from_link_tag2(&mut t);
}

fn run_test_rename_reference_from_link_tag2(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameReferenceFromLinkTag2") {
        return;
    }
    let content = r"// @Filename: /a.ts
enum E {
    /** {@link /**/Foo} */
    Foo
}
interface Foo {
    foo: E.Foo;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
