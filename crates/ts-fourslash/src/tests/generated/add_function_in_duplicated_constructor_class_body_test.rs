#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_function_in_duplicated_constructor_class_body() {
    let mut t = TestingT;
    run_test_add_function_in_duplicated_constructor_class_body(&mut t);
}

fn run_test_add_function_in_duplicated_constructor_class_body(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Foo {
    constructor() { }
    constructor() { }
    /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "fn() { }");
    f.verify_number_of_errors_in_current_file(2);
    done();
}
