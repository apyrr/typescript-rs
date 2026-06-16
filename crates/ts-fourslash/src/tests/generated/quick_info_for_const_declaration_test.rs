#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_const_declaration() {
    let mut t = TestingT;
    run_test_quick_info_for_const_declaration(&mut t);
}

fn run_test_quick_info_for_const_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const /**/c = 0 ;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "const c: 0", "");
    done();
}
