#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_selection_single_property() {
    let mut t = TestingT;
    run_test_format_selection_single_property(&mut t);
}

fn run_test_format_selection_single_property(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatSelectionSingleProperty") {
        return;
    }
    let content = r"console.log({
}, {
/*1*/    a: 1,
/*2*/    b: 2
})";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "1", "2");
    f.verify_current_file_content(
        t,
        r"console.log({
}, {
    a: 1,
    b: 2
})",
    );
    done();
}
