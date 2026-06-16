#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_global_augmentation1() {
    let mut t = TestingT;
    run_test_formatting_global_augmentation1(&mut t);
}

fn run_test_formatting_global_augmentation1(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingGlobalAugmentation1") {
        return;
    }
    let content = r"/*1*/declare          global                      {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "declare global {");
    done();
}
