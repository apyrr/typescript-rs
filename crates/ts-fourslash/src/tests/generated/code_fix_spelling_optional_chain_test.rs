#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling_optional_chain() {
    let mut t = TestingT;
    run_test_code_fix_spelling_optional_chain(&mut t);
}

fn run_test_code_fix_spelling_optional_chain(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixSpelling_optionalChain") {
        return;
    }
    let content = r"// @strict: true
function f(x: string | null) {
  [|x?.toLowrCase();|]
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "x?.toLowerCase();", false, 0, 0);
    done();
}
