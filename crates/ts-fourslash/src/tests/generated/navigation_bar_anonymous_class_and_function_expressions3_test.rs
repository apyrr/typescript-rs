#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_anonymous_class_and_function_expressions3() {
    let mut t = TestingT;
    run_test_navigation_bar_anonymous_class_and_function_expressions3(&mut t);
}

fn run_test_navigation_bar_anonymous_class_and_function_expressions3(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarAnonymousClassAndFunctionExpressions3") {
        return;
    }
    let content = r"describe('foo', () => {
    test(`a ${1} b ${2}`, () => {})
})

const a = 1;
const b = 2;
describe('foo', () => {
    test(`a ${a} b {b}`, () => {})
})";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
