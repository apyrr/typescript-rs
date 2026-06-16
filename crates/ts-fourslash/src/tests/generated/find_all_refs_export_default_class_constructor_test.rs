#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_export_default_class_constructor() {
    let mut t = TestingT;
    run_test_find_all_refs_export_default_class_constructor(&mut t);
}

fn run_test_find_all_refs_export_default_class_constructor(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsExportDefaultClassConstructor") {
        return;
    }
    let content = r"export default class {
    /*1*/constructor() {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
