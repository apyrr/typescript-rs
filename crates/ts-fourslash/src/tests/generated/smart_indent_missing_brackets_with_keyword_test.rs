#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_missing_brackets_with_keyword() {
    let mut t = TestingT;
    run_test_smart_indent_missing_brackets_with_keyword(&mut t);
}

fn run_test_smart_indent_missing_brackets_with_keyword(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"with /*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 0);
    done();
}
