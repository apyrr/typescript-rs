#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_from_link_tag_reference3() {
    let mut t = TestingT;
    run_test_find_all_references_from_link_tag_reference3(&mut t);
}

fn run_test_find_all_references_from_link_tag_reference3(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesFromLinkTagReference3") {
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
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
