#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_export_not_at_top_level() {
    let mut t = TestingT;
    run_test_find_all_refs_export_not_at_top_level(&mut t);
}

fn run_test_find_all_refs_export_not_at_top_level(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsExportNotAtTopLevel") {
        return;
    }
    let content = r"{
    /*1*/export const /*2*/x = 0;
    /*3*/x;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
