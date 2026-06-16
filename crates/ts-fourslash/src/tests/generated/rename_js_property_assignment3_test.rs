#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_property_assignment3() {
    let mut t = TestingT;
    run_test_rename_js_property_assignment3(&mut t);
}

fn run_test_rename_js_property_assignment3(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJsPropertyAssignment3") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.js
var C = class  {
}
[|C.[|{| "contextRangeIndex": 0 |}staticProperty|] = "string";|]
console.log(C.[|staticProperty|]);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "staticProperty");
    done();
}
