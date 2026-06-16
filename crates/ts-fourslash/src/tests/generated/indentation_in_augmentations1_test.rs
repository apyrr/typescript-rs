#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_indentation_in_augmentations1() {
    let mut t = TestingT;
    run_test_indentation_in_augmentations1(&mut t);
}

fn run_test_indentation_in_augmentations1(t: &mut TestingT) {
    if should_skip_if_failing("TestIndentationInAugmentations1") {
        return;
    }
    let content = r"// @module: amd
export {}
declare global {/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.verify_indentation(t, 4);
    done();
}
