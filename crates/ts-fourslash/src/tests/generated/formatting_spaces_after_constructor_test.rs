#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_spaces_after_constructor() {
    let mut t = TestingT;
    run_test_formatting_spaces_after_constructor(&mut t);
}

fn run_test_formatting_spaces_after_constructor(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/class test { constructor                   () { } }
/*2*/class test { constructor                   () { } }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "class test { constructor() { } }");
    {
        let mut opts = f.get_options();
        opts.format_code_settings.insert_space_after_constructor = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "class test { constructor () { } }");
    done();
}
