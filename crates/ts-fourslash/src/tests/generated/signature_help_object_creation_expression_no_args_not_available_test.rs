#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_object_creation_expression_no_args_not_available() {
    let mut t = TestingT;
    run_test_signature_help_object_creation_expression_no_args_not_available(&mut t);
}

fn run_test_signature_help_object_creation_expression_no_args_not_available(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class sampleCls { constructor(str: string, num: number) { } }
var x = new sampleCls/**/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &vec!["".to_string()]);
    done();
}
