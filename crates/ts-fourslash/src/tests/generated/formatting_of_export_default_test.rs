#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_of_export_default() {
    let mut t = TestingT;
    run_test_formatting_of_export_default(&mut t);
}

fn run_test_formatting_of_export_default(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOfExportDefault") {
        return;
    }
    let content = r"namespace Foo {
/*1*/    export        default        class        Test { }
}
/*2*/export        default        function        bar() { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    export default class Test { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "export default function bar() { }");
    done();
}
