#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_constructor_signature() {
    let mut t = TestingT;
    run_test_formatting_on_constructor_signature(&mut t);
}

fn run_test_formatting_on_constructor_signature(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnConstructorSignature") {
        return;
    }
    let content = r"/*1*/interface Gourai { new   () {} }
/*2*/type Stylet = { new   () {} }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "interface Gourai { new() { } }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "type Stylet = { new() { } }");
    done();
}
