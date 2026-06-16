#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_for_default_export05() {
    let mut t = TestingT;
    run_test_rename_for_default_export05(&mut t);
}

fn run_test_rename_for_default_export05(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: foo.ts
export default class DefaultExportedClass {
}
/*
 *  Commenting DefaultExportedClass
 */

var x: /**/[|DefaultExportedClass|];

var y = new DefaultExportedClass;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_rename_succeeded_at_current_position();
    done();
}
