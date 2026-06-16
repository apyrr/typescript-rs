#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_references_jsx_tag_name2() {
    let mut t = TestingT;
    run_test_find_references_jsx_tag_name2(&mut t);
}

fn run_test_find_references_jsx_tag_name2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: index.tsx
/*1*/const /*2*/obj = {Component: () => <div/>};
const element = </*3*/obj.Component/>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
