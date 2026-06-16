#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_binding_patterns() {
    let mut t = TestingT;
    run_test_smart_selection_binding_patterns(&mut t);
}

fn run_test_smart_selection_binding_patterns(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const { /*1*/x, y: /*2*/a, .../*3*/zs = {} } = {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
