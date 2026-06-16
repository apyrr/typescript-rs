#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_member21() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_member21(&mut t);
}

fn run_test_code_fix_add_missing_member21(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare let p: Promise<string>;
async function f() {
    p.toLowerCase();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixMissingMember".to_string()]);
    done();
}
