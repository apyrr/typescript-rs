#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_const_type_reference() {
    let mut t = TestingT;
    run_test_quick_info_for_const_type_reference(&mut t);
}

fn run_test_quick_info_for_const_type_reference(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForConstTypeReference") {
        return;
    }
    let content = r#""" as /**/const;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_not_quick_info_exists(t);
    done();
}
