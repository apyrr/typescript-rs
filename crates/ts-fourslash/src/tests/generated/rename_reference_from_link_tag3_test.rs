#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_reference_from_link_tag3() {
    let mut t = TestingT;
    run_test_rename_reference_from_link_tag3(&mut t);
}

fn run_test_rename_reference_from_link_tag3(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameReferenceFromLinkTag3") {
        return;
    }
    let content = r"// @filename: a.ts
interface Foo {
    foo: E.Foo;
}
// @Filename: b.ts
enum E {
    /** {@link /**/Foo} */
    Foo
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
