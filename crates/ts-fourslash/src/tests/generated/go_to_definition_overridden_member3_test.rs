#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_overridden_member3() {
    let mut t = TestingT;
    run_test_go_to_definition_overridden_member3(&mut t);
}

fn run_test_go_to_definition_overridden_member3(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionOverriddenMember3") {
        return;
    }
    let content = r"// @noImplicitOverride: true
abstract class Foo {
	abstract /*2*/m() {}
}

export class Bar extends Foo {
	[|/*1*/override|] m() {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
