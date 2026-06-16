#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_inside_with_block() {
    let mut t = TestingT;
    run_test_find_all_refs_inside_with_block(&mut t);
}

fn run_test_find_all_refs_inside_with_block(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/var /*2*/x = 0;

with ({}) {
    var y = x;  // Reference of x here should not be picked
    y++;        // also reference for y should be ignored
}

/*3*/x = /*4*/x + 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
