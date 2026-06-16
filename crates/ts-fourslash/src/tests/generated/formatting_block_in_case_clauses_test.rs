#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_block_in_case_clauses() {
    let mut t = TestingT;
    run_test_formatting_block_in_case_clauses(&mut t);
}

fn run_test_formatting_block_in_case_clauses(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingBlockInCaseClauses") {
        return;
    }
    let content = r"switch (1) {
    case 1:
        {
            /*1*/
        break;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "}");
    f.verify_current_line_content(t, "        }");
    done();
}
