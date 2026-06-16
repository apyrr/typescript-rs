#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_toggle_duplicate_function_declaration() {
    let mut t = TestingT;
    run_test_toggle_duplicate_function_declaration(&mut t);
}

fn run_test_toggle_duplicate_function_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class D { }
D();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_bof(t);
    f.insert(t, "declare function D();");
    f.go_to_bof(t);
    f.delete_at_caret(t, 21);
    done();
}
