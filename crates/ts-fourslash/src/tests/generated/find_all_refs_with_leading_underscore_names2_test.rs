#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_with_leading_underscore_names2() {
    let mut t = TestingT;
    run_test_find_all_refs_with_leading_underscore_names2(&mut t);
}

fn run_test_find_all_refs_with_leading_underscore_names2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Foo {
    /*1*/public /*2*/__bar() { return 0; }
}

var x: Foo;
x./*3*/__bar;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
