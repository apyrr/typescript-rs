#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_await_should_not_crash_if_not_in_function() {
    let mut t = TestingT;
    run_test_code_fix_await_should_not_crash_if_not_in_function(&mut t);
}

fn run_test_code_fix_await_should_not_crash_if_not_in_function(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAwaitShouldNotCrashIfNotInFunction") {
        return;
    }
    let content = r"await a";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["addMissingAwait".to_string()]);
    done();
}
