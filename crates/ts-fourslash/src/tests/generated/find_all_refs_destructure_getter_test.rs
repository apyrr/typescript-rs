#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_destructure_getter() {
    let mut t = TestingT;
    run_test_find_all_refs_destructure_getter(&mut t);
}

fn run_test_find_all_refs_destructure_getter(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsDestructureGetter") {
        return;
    }
    let content = r"class Test {
    get /*x0*/x() { return 0; }

    set /*y0*/y(a: number) {}
}
const { /*x1*/x, /*y1*/y } = new Test();
/*x2*/x; /*y2*/y;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "x0".to_string(),
            "x1".to_string(),
            "x2".to_string(),
            "y0".to_string(),
            "y1".to_string(),
            "y2".to_string(),
        ],
    );
    done();
}
