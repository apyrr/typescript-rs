#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_for_default_export09() {
    let mut t = TestingT;
    run_test_rename_for_default_export09(&mut t);
}

fn run_test_rename_for_default_export09(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameForDefaultExport09") {
        return;
    }
    let content = r"// @Filename: foo.ts
function /**/[|f|]() {
    return 100;
}

export default f;

var x: typeof f;

var y = f();

/**
 *  Commenting f
 */
namespace f {
    var local = 100;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_rename_succeeded_at_current_position();
    done();
}
