#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_var_from_module_with_reopened_enums() {
    let mut t = TestingT;
    run_test_remove_var_from_module_with_reopened_enums(&mut t);
}

fn run_test_remove_var_from_module_with_reopened_enums(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace A {
    /**/var o;
}
enum A {
}
enum A {
}
namespace A {
    var p;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.delete_at_caret(t, 6);
    done();
}
