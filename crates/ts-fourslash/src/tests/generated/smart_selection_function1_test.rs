#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_function1() {
    let mut t = TestingT;
    run_test_smart_selection_function1(&mut t);
}

fn run_test_smart_selection_function1(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_function1") {
        return;
    }
    let content = r"const f1 = () => {
   /**/
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
