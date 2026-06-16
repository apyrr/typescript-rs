#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unreachable_statement_node_reuse() {
    let mut t = TestingT;
    run_test_unreachable_statement_node_reuse(&mut t);
}

fn run_test_unreachable_statement_node_reuse(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function test() {
	return/*a*/abc();
	return;
}
function abc() { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "a");
    f.insert(t, " ");
    f.verify_no_errors();
    done();
}
