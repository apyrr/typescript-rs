#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_errors_after_resolving_variable_decl_of_merged_variable_and_class_decl() {
    let mut t = TestingT;
    run_test_errors_after_resolving_variable_decl_of_merged_variable_and_class_decl(&mut t);
}

fn run_test_errors_after_resolving_variable_decl_of_merged_variable_and_class_decl(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"namespace M {
    export class C {
        foo() { }
    }
    export namespace C {
        export var /*1*/C = M.C;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "1");
    f.backspace(t, 1);
    f.insert(t, " ");
    f.verify_quick_info_is(t, "var M.C.C: typeof M.C", "");
    f.verify_no_errors();
    done();
}
