#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_for_aliasing_export01() {
    let mut t = TestingT;
    run_test_rename_for_aliasing_export01(&mut t);
}

fn run_test_rename_for_aliasing_export01(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameForAliasingExport01") {
        return;
    }
    let content = r"// @Filename: foo.ts
let x = 1;

export { /**/[|x|] as y };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_rename_succeeded_at_current_position();
    done();
}
