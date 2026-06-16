#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling4() {
    let mut t = TestingT;
    run_test_code_fix_spelling4(&mut t);
}

fn run_test_code_fix_spelling4(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixSpelling4") {
        return;
    }
    let content = r"export declare const despite: { the: any };

[|dispite.the|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "despite.the", false, 0, 0);
    done();
}
