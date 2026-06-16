#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_object_types() {
    let mut t = TestingT;
    run_test_smart_selection_object_types(&mut t);
}

fn run_test_smart_selection_object_types(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_objectTypes") {
        return;
    }
    let content = r"type X = {
  /*1*/foo?: string;
  /*2*/readonly /*3*/bar: { x: num/*4*/ber };
  /*5*/meh
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
