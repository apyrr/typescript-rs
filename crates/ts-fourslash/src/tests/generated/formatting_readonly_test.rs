#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_readonly() {
    let mut t = TestingT;
    run_test_formatting_readonly(&mut t);
}

fn run_test_formatting_readonly(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
  readonly    property1: {};/*1*/
  public readonly   property2: {};/*2*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    readonly property1: {};");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    public readonly property2: {};");
    done();
}
