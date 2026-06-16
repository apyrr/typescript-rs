#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_on_list_end() {
    let mut t = TestingT;
    run_test_smart_indent_on_list_end(&mut t);
}

fn run_test_smart_indent_on_list_end(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentOnListEnd") {
        return;
    }
    let content = r#"var a = []
/*1*/
| {}
/*2*/
| "";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "2");
    f.verify_indentation(t, 4);
    done();
}
