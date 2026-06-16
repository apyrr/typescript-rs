#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_paste_lambda_over_module() {
    let mut t = TestingT;
    run_test_paste_lambda_over_module(&mut t);
}

fn run_test_paste_lambda_over_module(t: &mut TestingT) {
    if should_skip_if_failing("TestPasteLambdaOverModule") {
        return;
    }
    let content = r"// @strict: false
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.paste(t, "namespace B { }");
    f.go_to_bof(t);
    f.delete_at_caret(t, 15);
    f.insert(t, "var t = (public x) => { };");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
