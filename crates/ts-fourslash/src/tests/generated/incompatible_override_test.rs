#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incompatible_override() {
    let mut t = TestingT;
    run_test_incompatible_override(&mut t);
}

fn run_test_incompatible_override(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
class Foo { xyz: string; }
class Bar extends Foo { /*1*/xyz/*2*/: number = 1; }
class Baz extends Foo { public /*3*/xyz/*4*/: number = 2; }
class /*5*/Baf/*6*/ extends Foo {
   constructor(public xyz: number) {
      super();
   }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("3"), &f.marker_by_name("4"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("5"), &f.marker_by_name("6"), 0);
    f.verify_number_of_errors_in_current_file(3);
    done();
}
