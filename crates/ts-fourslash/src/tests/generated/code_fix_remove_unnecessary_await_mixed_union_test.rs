#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_remove_unnecessary_await_mixed_union() {
    let mut t = TestingT;
    run_test_code_fix_remove_unnecessary_await_mixed_union(&mut t);
}

fn run_test_code_fix_remove_unnecessary_await_mixed_union(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @target: esnext
async function fn1(a: Promise<void> | void) {
  await a;
}

async function fn2<T extends Promise<void> | void>(a: T) {
  await a;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
