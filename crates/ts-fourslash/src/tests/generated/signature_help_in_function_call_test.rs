#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_in_function_call() {
    let mut t = TestingT;
    run_test_signature_help_in_function_call(&mut t);
}

fn run_test_signature_help_in_function_call(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpInFunctionCall") {
        return;
    }
    let content = r"var items = [];
items.forEach(item => {
    for (/**/
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &vec!["".to_string()]);
    done();
}
