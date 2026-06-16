#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling_case_sensitive2() {
    let mut t = TestingT;
    run_test_code_fix_spelling_case_sensitive2(&mut t);
}

fn run_test_code_fix_spelling_case_sensitive2(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixSpellingCaseSensitive2") {
        return;
    }
    let content = r"export let console = 1;
export let Console = 1;
[|conole|] = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "console", false, 0, 0);
    done();
}
