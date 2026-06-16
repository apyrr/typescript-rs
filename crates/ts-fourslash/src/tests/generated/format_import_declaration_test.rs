#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_import_declaration() {
    let mut t = TestingT;
    run_test_format_import_declaration(&mut t);
}

fn run_test_format_import_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace Foo {/*1*/
}/*2*/

import bar  =    Foo;/*3*/

import bar2=Foo;/*4*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "namespace Foo {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "import bar = Foo;");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "import bar2 = Foo;");
    done();
}
