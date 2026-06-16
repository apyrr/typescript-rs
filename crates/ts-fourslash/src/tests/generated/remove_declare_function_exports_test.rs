#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_declare_function_exports() {
    let mut t = TestingT;
    run_test_remove_declare_function_exports(&mut t);
}

fn run_test_remove_declare_function_exports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare namespace M {
    function RegExp2(pattern: string): RegExp2;
    export function RegExp2(pattern: string, flags: string): RegExp2;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_bof(t);
    f.delete_at_caret(t, 8);
    done();
}
