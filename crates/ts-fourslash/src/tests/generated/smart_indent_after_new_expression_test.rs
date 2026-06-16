#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_after_new_expression() {
    let mut t = TestingT;
    run_test_smart_indent_after_new_expression(&mut t);
}

fn run_test_smart_indent_after_new_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"
new Array
{| "indent": 0 |}
new Array;
{| "indent": 0 |}
new Array(0);
{| "indent": 0 |}
new Array(;
{| "indent": 0 |}
new Array(
{| "indent": 4 |}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_indentation_at_markers_from_data(t);
    done();
}
