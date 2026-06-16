#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_consistent_contextual_type_errors_after_edits() {
    let mut t = TestingT;
    run_test_consistent_contextual_type_errors_after_edits(&mut t);
}

fn run_test_consistent_contextual_type_errors_after_edits(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
class A {
    foo: string;
}
class C {
    foo: string;
}
var xs /*1*/ = [(x: A) => { return x.foo; }, (x: C) => { return x.foo; }];
xs.forEach(y => y(new /*2*/A()));";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_marker(t, "1");
    f.insert(t, ": {}[]");
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "2");
    f.delete_at_caret(t, 1);
    f.insert(t, "C");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
