#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_statement_for() {
    let mut t = TestingT;
    run_test_smart_indent_statement_for(&mut t);
}

fn run_test_smart_indent_statement_for(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentStatementFor") {
        return;
    }
    let content = r"function Foo() {
    for (var i = 0; i < 10; i++) {
        /*insideStatement*/
    }
    /*afterStatement*/
    for (var i = 0;;) 
        /*insideStatement2*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "insideStatement");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "afterStatement");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "insideStatement2");
    f.verify_indentation(t, 8);
    done();
}
