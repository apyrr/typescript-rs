#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_properties_of_generic_type() {
    let mut t = TestingT;
    run_test_references_for_properties_of_generic_type(&mut t);
}

fn run_test_references_for_properties_of_generic_type(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForPropertiesOfGenericType") {
        return;
    }
    let content = r#"interface IFoo<T> {
    /*1*/doSomething(v: T): T;
}

var x: IFoo<string>;
x./*2*/doSomething("ss");

var y: IFoo<number>;
y./*3*/doSomething(12);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
