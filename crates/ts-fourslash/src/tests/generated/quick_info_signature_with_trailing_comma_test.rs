#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_signature_with_trailing_comma() {
    let mut t = TestingT;
    run_test_quick_info_signature_with_trailing_comma(&mut t);
}

fn run_test_quick_info_signature_with_trailing_comma(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare function f<T>(a: T): T;
/**/f(2,);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "function f<2>(a: 2): 2", "");
    done();
}
