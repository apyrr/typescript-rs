#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_type_alias() {
    let mut t = TestingT;
    run_test_format_type_alias(&mut t);
}

fn run_test_format_type_alias(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type   Alias = /*typeKeyword*/
/*indent*/
number;/*autoformat*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "typeKeyword");
    f.verify_current_line_content(t, "type Alias =");
    f.go_to_marker(t, "indent");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "autoformat");
    f.verify_current_line_content(t, "    number;");
    done();
}
