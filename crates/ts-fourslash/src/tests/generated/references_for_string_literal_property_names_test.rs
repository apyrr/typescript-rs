#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_string_literal_property_names() {
    let mut t = TestingT;
    run_test_references_for_string_literal_property_names(&mut t);
}

fn run_test_references_for_string_literal_property_names(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class Foo {
    public "/*1*/ss": any;
}

var x: Foo;
x.ss;
x["ss"];
x = { "ss": 0 };
x = { ss: 0 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
