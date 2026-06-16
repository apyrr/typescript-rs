#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_prototype_property02() {
    let mut t = TestingT;
    run_test_rename_js_prototype_property02(&mut t);
}

fn run_test_rename_js_prototype_property02(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJsPrototypeProperty02") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.js
function bar() {
}
[|bar.prototype.[|{| "contextRangeIndex": 0 |}x|] = 10;|]
var t = new bar();
[|t.[|{| "contextRangeIndex": 2 |}x|] = 11;|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "x");
    done();
}
