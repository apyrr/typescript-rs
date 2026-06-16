#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_extend_array_interface_member() {
    let mut t = TestingT;
    run_test_extend_array_interface_member(&mut t);
}

fn run_test_extend_array_interface_member(t: &mut TestingT) {
    if should_skip_if_failing("TestExtendArrayInterfaceMember") {
        return;
    }
    let content = r"// @strict: false
var x = [1, 2, 3];
var /*y*/y = x.pop(/*1*/5/*2*/);
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"));
    f.verify_number_of_errors_in_current_file(1);
    f.verify_quick_info_at(t, "y", "var y: number", "");
    f.go_to_eof(t);
    f.insert(t, "interface Array<T> { pop(def: T): T; }");
    f.verify_no_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"));
    f.verify_quick_info_at(t, "y", "var y: number", "");
    f.verify_no_errors();
    done();
}
