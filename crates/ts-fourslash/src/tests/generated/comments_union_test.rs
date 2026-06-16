#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_comments_union() {
    let mut t = TestingT;
    run_test_comments_union(&mut t);
}

fn run_test_comments_union(t: &mut TestingT) {
    if should_skip_if_failing("TestCommentsUnion") {
        return;
    }
    let content = r"var a: Array<string> | Array<number>;
a./*1*/length";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) Array<T>.length: number", "Gets or sets the length of the array. This is a number one higher than the highest index in the array.");
    done();
}
