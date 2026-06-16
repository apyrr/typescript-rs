#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_rest() {
    let mut t = TestingT;
    run_test_find_all_refs_for_rest(&mut t);
}

fn run_test_find_all_refs_for_rest(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForRest") {
        return;
    }
    let content = r"interface Gen {
    x: number
    /*1*/parent: Gen;
    millenial: string;
}
let t: Gen;
var { x, ...rest } = t;
rest./*2*/parent;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
