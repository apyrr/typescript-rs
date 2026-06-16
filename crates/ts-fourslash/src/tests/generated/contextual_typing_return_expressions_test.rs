#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextual_typing_return_expressions() {
    let mut t = TestingT;
    run_test_contextual_typing_return_expressions(&mut t);
}

fn run_test_contextual_typing_return_expressions(t: &mut TestingT) {
    if should_skip_if_failing("TestContextualTypingReturnExpressions") {
        return;
    }
    let content = r"interface A { }
var f44: (x: A) => (y: A) => A = /*1*/x => /*2*/y => /*3*/x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) x: A", "");
    f.verify_quick_info_at(t, "2", "(parameter) y: A", "");
    f.verify_quick_info_at(t, "3", "(parameter) x: A", "");
    done();
}
