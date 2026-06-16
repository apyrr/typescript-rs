#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling_case_sensitive3() {
    let mut t = TestingT;
    run_test_code_fix_spelling_case_sensitive3(&mut t);
}

fn run_test_code_fix_spelling_case_sensitive3(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixSpellingCaseSensitive3") {
        return;
    }
    let content = r"class Node {}
let node = new Node();
[|nodes|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "node", false, 0, 0);
    done();
}
