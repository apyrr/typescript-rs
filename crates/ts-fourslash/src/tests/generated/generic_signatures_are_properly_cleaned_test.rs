#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_signatures_are_properly_cleaned() {
    let mut t = TestingT;
    run_test_generic_signatures_are_properly_cleaned(&mut t);
}

fn run_test_generic_signatures_are_properly_cleaned(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericSignaturesAreProperlyCleaned") {
        return;
    }
    let content = r"interface Int<T> {
val<U>(f: (t: T) => U): Int<U>;
}
declare var v1: Int<string>;
var v2: Int<number> = v1/*1*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "1");
    f.delete_at_caret(t, 1);
    f.verify_number_of_errors_in_current_file(1);
    done();
}
