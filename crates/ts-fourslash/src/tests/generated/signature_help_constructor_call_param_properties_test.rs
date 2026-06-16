#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_constructor_call_param_properties() {
    let mut t = TestingT;
    run_test_signature_help_constructor_call_param_properties(&mut t);
}

fn run_test_signature_help_constructor_call_param_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpConstructorCallParamProperties") {
        return;
    }
    let content = r"class Circle {
    /**
      * Initialize a circle.
      * @param  radius The radius of the circle.
      */
    constructor(private radius: number) {
    }
}
var a = new Circle(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
