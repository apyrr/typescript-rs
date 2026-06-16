#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_of_constructor_with_modifier() {
    let mut t = TestingT;
    run_test_find_all_refs_of_constructor_with_modifier(&mut t);
}

fn run_test_find_all_refs_of_constructor_with_modifier(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsOfConstructor_withModifier") {
        return;
    }
    let content = r"class X {
    public /*0*/constructor() {}
}
var x = new X();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string()]);
    done();
}
