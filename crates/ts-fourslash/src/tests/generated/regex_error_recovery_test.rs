#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_regex_error_recovery() {
    let mut t = TestingT;
    run_test_regex_error_recovery(&mut t);
}

fn run_test_regex_error_recovery(t: &mut TestingT) {
    if should_skip_if_failing("TestRegexErrorRecovery") {
        return;
    }
    let content = r#" // test code
//var x = //**/a/;/*1*/
//x.exec("bab");
 Bug 579071: Parser no longer detects a Regex when an open bracket is inserted
verify.quickInfoIs("RegExp");
verify.not.errorExistsAfterMarker("1");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.insert(t, "(");
    done();
}
