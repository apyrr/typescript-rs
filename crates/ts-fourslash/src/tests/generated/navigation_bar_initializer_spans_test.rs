#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_initializer_spans() {
    let mut t = TestingT;
    run_test_navigation_bar_initializer_spans(&mut t);
}

fn run_test_navigation_bar_initializer_spans(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarInitializerSpans") {
        return;
    }
    let content = r"// get the name for the navbar from the variable name rather than the function name
const [|[|x|] = () => { var [|a|]; }|];
const [|[|f|] = function f() { var [|b|]; }|];
const [|[|y|] = { [|[|z|]: function z() { var [|c|]; }|] }|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
