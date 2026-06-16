#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_duplicate_identifier() {
    let mut t = TestingT;
    run_test_remove_duplicate_identifier(&mut t);
}

fn run_test_remove_duplicate_identifier(t: &mut TestingT) {
    if should_skip_if_failing("TestRemoveDuplicateIdentifier") {
        return;
    }
    let content = r"class foo{}
function foo() { return null; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_bof(t);
    f.delete_at_caret(t, 11);
    done();
}
