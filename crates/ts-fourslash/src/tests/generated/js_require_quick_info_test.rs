#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_require_quick_info() {
    let mut t = TestingT;
    run_test_js_require_quick_info(&mut t);
}

fn run_test_js_require_quick_info(t: &mut TestingT) {
    if should_skip_if_failing("TestJsRequireQuickInfo") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.js
const /**/x = require("./b");
// @Filename: b.js
exports.x = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "import x", "");
    done();
}
