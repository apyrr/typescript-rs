#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_assertions_formatting() {
    let mut t = TestingT;
    run_test_type_assertions_formatting(&mut t);
}

fn run_test_type_assertions_formatting(t: &mut TestingT) {
    if should_skip_if_failing("TestTypeAssertionsFormatting") {
        return;
    }
    let content = r"( <  any   >      publisher);/*1*/
 <  any  >      3;/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "(<any>publisher);");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "<any>3;");
    done();
}
