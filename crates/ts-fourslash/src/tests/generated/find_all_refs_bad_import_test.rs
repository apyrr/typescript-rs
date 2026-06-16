#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_bad_import() {
    let mut t = TestingT;
    run_test_find_all_refs_bad_import(&mut t);
}

fn run_test_find_all_refs_bad_import(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsBadImport") {
        return;
    }
    let content = r#"import { /*0*/ab as /*1*/cd } from "doesNotExist";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string()]);
    done();
}
