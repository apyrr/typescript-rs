#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_comments() {
    let mut t = TestingT;
    run_test_format_comments(&mut t);
}

fn run_test_format_comments(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatComments") {
        return;
    }
    let content = r"_.chain()
// wow/*callChain1*/
  .then()
// waa/*callChain2*/
    .then();
wow(
  3,
// uaa/*argument1*/
    4
// wua/*argument2*/
);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "callChain1");
    f.verify_current_line_content(t, "    // wow");
    f.go_to_marker(t, "callChain2");
    f.verify_current_line_content(t, "    // waa");
    f.go_to_marker(t, "argument1");
    f.verify_current_line_content(t, "    // uaa");
    f.go_to_marker(t, "argument2");
    f.verify_current_line_content(t, "    // wua");
    done();
}
