#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_exports01() {
    let mut t = TestingT;
    run_test_rename_js_exports01(&mut t);
}

fn run_test_rename_js_exports01(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJsExports01") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.js
[|exports.[|{| "contextRangeIndex": 0 |}area|] = function (r) { return r * r; }|]
// @Filename: b.js
var mod = require('./a');
var t = mod./*1*/[|area|](10);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    f.verify_baseline_rename_at_ranges_with_text(t, "area");
    done();
}
