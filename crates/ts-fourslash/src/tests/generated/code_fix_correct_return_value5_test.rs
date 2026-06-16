#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_correct_return_value5() {
    let mut t = TestingT;
    run_test_code_fix_correct_return_value5(&mut t);
}

fn run_test_code_fix_correct_return_value5(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixCorrectReturnValue5") {
        return;
    }
    let content = r"function Foo (): void {
    undefined
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
