#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_declare_param_type_annotation() {
    let mut t = TestingT;
    run_test_remove_declare_param_type_annotation(&mut t);
}

fn run_test_remove_declare_param_type_annotation(t: &mut TestingT) {
    if should_skip_if_failing("TestRemoveDeclareParamTypeAnnotation") {
        return;
    }
    let content = r"declare class T { }
declare function parseInt(/**/s:T):T;
parseInt('2');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.delete_at_caret(t, 3);
    done();
}
