#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_inherited_properties6() {
    let mut t = TestingT;
    run_test_references_for_inherited_properties6(&mut t);
}

fn run_test_references_for_inherited_properties6(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForInheritedProperties6") {
        return;
    }
    let content = r"class class1 extends class1 {
    /*1*/doStuff() { }
}
class class2 extends class1 {
    doStuff() { }
}

var v: class2;
v.doStuff();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
