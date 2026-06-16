#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_optional_param15() {
    let mut t = TestingT;
    run_test_code_fix_add_optional_param15(&mut t);
}

fn run_test_code_fix_add_optional_param15(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAddOptionalParam15") {
        return;
    }
    let content = r"function f(a: number, b: number) {}
f();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["addOptionalParam".to_string()]);
    done();
}
