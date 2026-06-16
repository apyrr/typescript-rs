#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_delete_type_parameter() {
    let mut t = TestingT;
    run_test_delete_type_parameter(&mut t);
}

fn run_test_delete_type_parameter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Query<T> {
    groupBy(): Query</**/T>;
}
interface Query2<T> {
    groupBy(): Query2<Query<T>>;
}
var q1: Query<number>;
var q2: Query2<number>;
q1 = q2;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.delete_at_caret(t, 1);
    done();
}
