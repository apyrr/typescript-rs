#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_import_meta() {
    let mut t = TestingT;
    run_test_find_all_references_import_meta(&mut t);
}

fn run_test_find_all_references_import_meta(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesImportMeta") {
        return;
    }
    let content = r"// Haha that's so meta!

let x = import.meta/**/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
