#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_async_computed_method() {
    let mut t = TestingT;
    run_test_format_async_computed_method(&mut t);
}

fn run_test_format_async_computed_method(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatAsyncComputedMethod") {
        return;
    }
    let content = r"class C {
    /*method*/async [0]() { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "method");
    f.verify_current_line_content(t, "    async [0]() { }");
    done();
}
