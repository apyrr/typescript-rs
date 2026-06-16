#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_negative_tests() {
    let mut t = TestingT;
    run_test_signature_help_negative_tests(&mut t);
}

fn run_test_signature_help_negative_tests(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//inside a comment foo(/*insideComment*/
cl/*invalidContext*/ass InvalidSignatureHelpLocation { }
InvalidSignatureHelpLocation(/*validContext*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(
        t,
        &vec![
            "insideComment".to_string(),
            "invalidContext".to_string(),
            "validContext".to_string(),
        ],
    );
    done();
}
