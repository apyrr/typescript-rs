#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_goto_definition_link_tag4() {
    let mut t = TestingT;
    run_test_goto_definition_link_tag4(&mut t);
}

fn run_test_goto_definition_link_tag4(t: &mut TestingT) {
    if should_skip_if_failing("TestGotoDefinitionLinkTag4") {
        return;
    }
    let content = r"// @filename: a.ts
interface [|/*2*/Foo|] {
    foo: E.Foo;
}
// @Filename: b.ts
enum E {
    /** {@link /*1*/[|Foo|]} */
    Foo
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
