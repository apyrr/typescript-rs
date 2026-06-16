#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_from_empty_block_comment() {
    let mut t = TestingT;
    run_test_quick_info_from_empty_block_comment(&mut t);
}

fn run_test_quick_info_from_empty_block_comment(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoFromEmptyBlockComment") {
        return;
    }
    let content = r"/**/
class Foo {
}
var f/*A*/ff = new Foo();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "A", "var fff: Foo", "");
    done();
}
