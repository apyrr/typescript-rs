#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_rest_args3() {
    let mut t = TestingT;
    run_test_signature_help_rest_args3(&mut t);
}

fn run_test_signature_help_rest_args3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @target: esnext
// @lib: esnext
const layers = Object.assign({}, /*1*/...[]);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
