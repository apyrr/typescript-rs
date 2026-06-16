#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_no_variable_type_hints() {
    let mut t = TestingT;
    run_test_inlay_hints_no_variable_type_hints(&mut t);
}

fn run_test_inlay_hints_no_variable_type_hints(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const a = 123;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
