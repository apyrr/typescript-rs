#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_check_object_in_array_literal() {
    let mut t = TestingT;
    run_test_type_check_object_in_array_literal(&mut t);
}

fn run_test_type_check_object_in_array_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare function create<T>(initialValues);
create([{}]);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_position(t, 0);
    f.insert(t, "");
    done();
}
