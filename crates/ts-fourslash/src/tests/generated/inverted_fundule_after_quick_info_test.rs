#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inverted_fundule_after_quick_info() {
    let mut t = TestingT;
    run_test_inverted_fundule_after_quick_info(&mut t);
}

fn run_test_inverted_fundule_after_quick_info(t: &mut TestingT) {
    if should_skip_if_failing("TestInvertedFunduleAfterQuickInfo") {
        return;
    }
    let content = r"namespace M {
    namespace A {
        var o;
    }
    function A(/**/x: number): void { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    f.verify_number_of_errors_in_current_file(1);
    done();
}
