#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_await_top_level() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_await_top_level(&mut t);
}

fn run_test_code_fix_add_missing_await_top_level(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAddMissingAwait_topLevel") {
        return;
    }
    let content = r"declare function getPromise(): Promise<string>;
const p = getPromise();
while (true) {
  p/*0*/.toLowerCase();
  getPromise()/*1*/.toLowerCase();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["addMissingAwait".to_string()]);
    f.verify_code_fix_not_available(t, &vec!["addMissingAwaitToInitializer".to_string()]);
    done();
}
