#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextual_typing_of_generic_call_signatures2() {
    let mut t = TestingT;
    run_test_contextual_typing_of_generic_call_signatures2(&mut t);
}

fn run_test_contextual_typing_of_generic_call_signatures2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    <T>(x: T): void
}
function f6(x: <T extends I>(p: T) => void) { }
// x should not be contextually typed so this should be an error
f6(/**/x => x<number>())";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(parameter) x: T extends I", "");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
