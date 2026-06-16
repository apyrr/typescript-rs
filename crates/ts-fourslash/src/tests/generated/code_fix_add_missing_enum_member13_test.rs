#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_enum_member13() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_enum_member13(&mut t);
}

fn run_test_code_fix_add_missing_enum_member13(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"enum E { A, B }
declare var a: E;
a.C;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixMissingMember".to_string()]);
    done();
}
