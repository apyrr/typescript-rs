#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_all_missing_imports_no_crash2() {
    let mut t = TestingT;
    run_test_add_all_missing_imports_no_crash2(&mut t);
}

fn run_test_add_all_missing_imports_no_crash2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: file1.ts
export { /**/default };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_code_fix_all_not_available(t, "fixMissingImport");
    done();
}
