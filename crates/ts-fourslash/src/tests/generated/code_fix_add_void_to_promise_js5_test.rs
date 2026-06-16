#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_void_to_promise_js5() {
    let mut t = TestingT;
    run_test_code_fix_add_void_to_promise_js5(&mut t);
}

fn run_test_code_fix_add_void_to_promise_js5(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAddVoidToPromiseJS5") {
        return;
    }
    let content = r"// @target: esnext
// @lib: es2015
// @strict: true
// @allowJS: true
// @checkJS: true
// @filename: main.js
/** @type {Promise<number>} */
const p2 = new Promise(resolve => resolve());";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(
        t,
        &vec!["Add 'void' to Promise resolved without a value".to_string()],
    );
    done();
}
