#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unknown_variable_error_recovery() {
    let mut t = TestingT;
    run_test_unknown_variable_error_recovery(&mut t);
}

fn run_test_unknown_variable_error_recovery(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var foo = [1, 2, 3];
for (var bar = 0; foo[bar] < 5; bear/**/++ ) {
    foo[bar] = 0;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_error_exists_after_marker_name("");
    done();
}
