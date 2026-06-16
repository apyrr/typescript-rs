#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_on_unclosed_object_type_literal01() {
    let mut t = TestingT;
    run_test_smart_indent_on_unclosed_object_type_literal01(&mut t);
}

fn run_test_smart_indent_on_unclosed_object_type_literal01(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentOnUnclosedObjectTypeLiteral01") {
        return;
    }
    let content = r#"var x: {
{| "indent": 4 |}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_indentation_at_markers_from_data(t);
    done();
}
