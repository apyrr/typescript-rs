#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_equals() {
    let mut t = TestingT;
    run_test_find_all_refs_import_equals(&mut t);
}

fn run_test_find_all_refs_import_equals(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsImportEquals") {
        return;
    }
    let content = r"import j = N./**/q;
namespace N { export const q = 0; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
