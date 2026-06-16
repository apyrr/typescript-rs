#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_templates() {
    let mut t = TestingT;
    run_test_formatting_templates(&mut t);
}

fn run_test_formatting_templates(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingTemplates") {
        return;
    }
    let content = r"String.call `${123}`/*1*/
String.call `${123} ${456}`/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, ";");
    f.verify_current_line_content(t, "String.call`${123}`;");
    f.go_to_marker(t, "2");
    f.insert(t, ";");
    f.verify_current_line_content(t, "String.call`${123} ${456}`;");
    done();
}
