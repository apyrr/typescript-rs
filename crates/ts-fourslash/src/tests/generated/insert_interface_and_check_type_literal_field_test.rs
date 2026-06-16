#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_insert_interface_and_check_type_literal_field() {
    let mut t = TestingT;
    run_test_insert_interface_and_check_type_literal_field(&mut t);
}

fn run_test_insert_interface_and_check_type_literal_field(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*addC*/
interface G<T, U> { }
var v2: G<{ a: /*checkParam*/C }, C>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "addC");
    f.insert(t, "interface C { }");
    f.go_to_marker(t, "checkParam");
    f.verify_quick_info_exists(t);
    done();
}
