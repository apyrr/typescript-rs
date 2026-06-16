#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_in_invalid_index_signature() {
    let mut t = TestingT;
    run_test_quick_info_in_invalid_index_signature(&mut t);
}

fn run_test_quick_info_in_invalid_index_signature(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInInvalidIndexSignature") {
        return;
    }
    let content = r"function method() { var /**/dictionary = <{ [index]: string; }>{}; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "",
        "(local var) dictionary: {\n    [x: number]: string;\n}",
        "",
    );
    done();
}
