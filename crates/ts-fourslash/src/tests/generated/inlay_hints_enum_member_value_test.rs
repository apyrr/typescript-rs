#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_enum_member_value() {
    let mut t = TestingT;
    run_test_inlay_hints_enum_member_value(&mut t);
}

fn run_test_inlay_hints_enum_member_value(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"enum E {
    A,
    AA,
    B = 10,
    BB,
    C = 'C',
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
