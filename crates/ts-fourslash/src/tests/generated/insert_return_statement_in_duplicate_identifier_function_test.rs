#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_insert_return_statement_in_duplicate_identifier_function() {
    let mut t = TestingT;
    run_test_insert_return_statement_in_duplicate_identifier_function(&mut t);
}

fn run_test_insert_return_statement_in_duplicate_identifier_function(t: &mut TestingT) {
    if should_skip_if_failing("TestInsertReturnStatementInDuplicateIdentifierFunction") {
        return;
    }
    let content = r"// @strict: true
class foo { };
function foo() { /**/ }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_number_of_errors_in_current_file(2);
    f.insert(t, "return null;");
    f.verify_number_of_errors_in_current_file(2);
    done();
}
