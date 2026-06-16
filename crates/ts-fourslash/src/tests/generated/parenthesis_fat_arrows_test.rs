#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_parenthesis_fat_arrows() {
    let mut t = TestingT;
    run_test_parenthesis_fat_arrows(&mut t);
}

fn run_test_parenthesis_fat_arrows(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
x => x;
(y) => y;
/**/
(y) => y;
x => x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_no_error_exists_before_marker_name("");
    f.verify_no_error_exists_after_marker_name("");
    done();
}
