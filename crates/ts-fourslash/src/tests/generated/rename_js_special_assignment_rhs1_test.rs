#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_special_assignment_rhs1() {
    let mut t = TestingT;
    run_test_rename_js_special_assignment_rhs1(&mut t);
}

fn run_test_rename_js_special_assignment_rhs1(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJsSpecialAssignmentRhs1") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: a.js
const foo = {
    set: function (x) {
        this._x = x;
    },
    copy: function ([|x|]) {
        this._x = [|x|].prop;
    }
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &[]);
    done();
}
