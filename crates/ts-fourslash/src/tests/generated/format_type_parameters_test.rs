#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_type_parameters() {
    let mut t = TestingT;
    run_test_format_type_parameters(&mut t);
}

fn run_test_format_type_parameters(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatTypeParameters") {
        return;
    }
    let content = r"/**/type Bar<T extends any[]= any[]> = T";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "");
    f.verify_current_line_content(t, "type Bar<T extends any[] = any[]> = T");
    done();
}
