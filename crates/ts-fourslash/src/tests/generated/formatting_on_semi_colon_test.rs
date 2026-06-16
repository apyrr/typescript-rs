#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_semi_colon() {
    let mut t = TestingT;
    run_test_formatting_on_semi_colon(&mut t);
}

fn run_test_formatting_on_semi_colon(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnSemiColon") {
        return;
    }
    let content = r"var  a=b+c^d-e*++f";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_eof(t);
    f.insert(t, ";");
    f.verify_current_file_content(t, r"var a = b + c ^ d - e * ++f;");
    done();
}
