#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling_short_name2() {
    let mut t = TestingT;
    run_test_code_fix_spelling_short_name2(&mut t);
}

fn run_test_code_fix_spelling_short_name2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"export let ab = 1;
abc;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
