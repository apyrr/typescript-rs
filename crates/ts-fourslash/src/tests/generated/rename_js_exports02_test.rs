#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_exports02() {
    let mut t = TestingT;
    run_test_rename_js_exports02(&mut t);
}

fn run_test_rename_js_exports02(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJsExports02") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.js
module.exports = class /*1*/A {}
// @Filename: b.js
const /*2*/A = require("./a");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
