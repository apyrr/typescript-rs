#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_class_expression() {
    let mut t = TestingT;
    run_test_format_class_expression(&mut t);
}

fn run_test_format_class_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Thing extends (
    class/*classOpenBrace*/
    {
/*classIndent*/
    protected  doThing() {/*methodAutoformat*/
/*methodIndent*/
    }
    }
) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "classOpenBrace");
    f.verify_current_line_content(t, "    class {");
    f.go_to_marker(t, "classIndent");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "methodAutoformat");
    f.verify_current_line_content(t, "        protected doThing() {");
    f.go_to_marker(t, "methodIndent");
    f.verify_indentation(t, 12);
    done();
}
