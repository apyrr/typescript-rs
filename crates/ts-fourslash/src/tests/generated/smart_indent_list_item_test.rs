#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_list_item() {
    let mut t = TestingT;
    run_test_smart_indent_list_item(&mut t);
}

fn run_test_smart_indent_list_item(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"[1,
    2
          + 3, 4,
    /*1*/
[1,
    2
          + 3, 4
    /*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "2");
    f.verify_indentation(t, 4);
    done();
}
