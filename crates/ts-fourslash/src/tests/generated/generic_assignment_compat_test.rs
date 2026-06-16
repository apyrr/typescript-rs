#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_assignment_compat() {
    let mut t = TestingT;
    run_test_generic_assignment_compat(&mut t);
}

fn run_test_generic_assignment_compat(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Int<T> {

    val<U>(f: (t: T) => U): Int<U>;

}

declare var v1: Int<string>;

var /*1*/v2/*2*/: Int<number> = v1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"), 0);
    f.verify_number_of_errors_in_current_file(1);
    done();
}
