#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_with_leading_underscore_names5() {
    let mut t = TestingT;
    run_test_find_all_refs_with_leading_underscore_names5(&mut t);
}

fn run_test_find_all_refs_with_leading_underscore_names5(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsWithLeadingUnderscoreNames5") {
        return;
    }
    let content = r"class Foo {
    public _bar;
    public __bar;
    /*1*/public /*2*/___bar;
    public ____bar;
}

var x: Foo;
x._bar;
x.__bar;
x./*3*/___bar;
x.____bar;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
