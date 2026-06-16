#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_module_indentation() {
    let mut t = TestingT;
    run_test_formatting_on_module_indentation(&mut t);
}

fn run_test_formatting_on_module_indentation(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"  namespace     Foo    {
    export    namespace    A  .   B  .   C     {      }/**/
               }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_bof(t);
    f.verify_current_line_content(t, "namespace Foo {");
    f.go_to_marker(t, "");
    f.verify_current_line_content(t, "    export namespace A.B.C { }");
    f.go_to_eof(t);
    f.verify_current_line_content(t, "}");
    done();
}
