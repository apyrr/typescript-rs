#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_above_number_literal_expression_statement() {
    let mut t = TestingT;
    run_test_type_above_number_literal_expression_statement(&mut t);
}

fn run_test_type_above_number_literal_expression_statement(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"
// foo
1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_bof(t);
    f.insert(t, "var x;\n");
    done();
}
