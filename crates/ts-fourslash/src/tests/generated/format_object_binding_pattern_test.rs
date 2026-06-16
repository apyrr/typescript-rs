#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_object_binding_pattern() {
    let mut t = TestingT;
    run_test_format_object_binding_pattern(&mut t);
}

fn run_test_format_object_binding_pattern(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const {
x,
y,
} = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"const {
    x,
    y,
} = 0;",
    );
    done();
}
