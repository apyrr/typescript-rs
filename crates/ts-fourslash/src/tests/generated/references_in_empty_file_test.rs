#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_in_empty_file() {
    let mut t = TestingT;
    run_test_references_in_empty_file(&mut t);
}

fn run_test_references_in_empty_file(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesInEmptyFile") {
        return;
    }
    let content = r"// @lib: es5
/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
