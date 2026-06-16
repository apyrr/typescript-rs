#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unclosed_string_literal_autoformating() {
    let mut t = TestingT;
    run_test_unclosed_string_literal_autoformating(&mut t);
}

fn run_test_unclosed_string_literal_autoformating(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var x = /*1*/"asd/*2*/
class Foo {
    /**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "}");
    f.verify_current_line_content(t, "}");
    done();
}
