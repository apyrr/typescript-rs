#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_re_export_broken2() {
    let mut t = TestingT;
    run_test_find_all_refs_re_export_broken2(&mut t);
}

fn run_test_find_all_refs_re_export_broken2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
/*1*/export { /*2*/x } from "nonsense";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
