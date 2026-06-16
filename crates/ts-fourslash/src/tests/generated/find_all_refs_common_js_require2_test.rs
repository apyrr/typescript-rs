#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_common_js_require2() {
    let mut t = TestingT;
    run_test_find_all_refs_common_js_require2(&mut t);
}

fn run_test_find_all_refs_common_js_require2(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsCommonJsRequire2") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
function f() { }
module.exports.f = f
// @Filename: /b.js
const { f } = require('./a')
/**/f";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
