#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_primitive_js_doc() {
    let mut t = TestingT;
    run_test_find_all_refs_primitive_js_doc(&mut t);
}

fn run_test_find_all_refs_primitive_js_doc(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsPrimitiveJsDoc") {
        return;
    }
    let content = r"// @noLib: true
/**
 * @param {/*1*/number} n
 * @returns {/*2*/number}
 */
function f(n: /*3*/number): /*4*/number {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
