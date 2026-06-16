#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_signature_partial() {
    let mut t = TestingT;
    run_test_add_signature_partial(&mut t);
}

fn run_test_add_signature_partial(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.insert(t, "interface Number { toFixed");
    f.insert(t, "(");
    done();
}
