#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_function_declaration20() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_function_declaration20(&mut t);
}

fn run_test_code_fix_add_missing_function_declaration20(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const a = {
   b: { f(x: number) {} }
}
a.b.f(foo);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixMissingFunctionDeclaration".to_string()]);
    done();
}
