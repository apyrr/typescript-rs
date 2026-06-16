#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_property_duplicate_identifier_error() {
    let mut t = TestingT;
    run_test_property_duplicate_identifier_error(&mut t);
}

fn run_test_property_duplicate_identifier_error(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"export class C {
    x: number;
    get x(): number { return 1; }
}/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "/n");
    done();
}
