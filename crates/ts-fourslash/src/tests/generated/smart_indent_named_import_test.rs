#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_named_import() {
    let mut t = TestingT;
    run_test_smart_indent_named_import(&mut t);
}

fn run_test_smart_indent_named_import(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentNamedImport") {
        return;
    }
    let content = r"import {/*0*/
    numbers as bn,/*1*/
    list/*2*/
} from '@bykov/basics';/*3*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "0");
    f.verify_current_line_content(t, "import {");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    numbers as bn,");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    list");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "} from '@bykov/basics';");
    done();
}
