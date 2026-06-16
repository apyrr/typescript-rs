#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_this_property03() {
    let mut t = TestingT;
    run_test_rename_js_this_property03(&mut t);
}

fn run_test_rename_js_this_property03(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @Filename: a.js
class C {
  constructor(y) {
    [|this.[|{| "contextRangeIndex": 0 |}x|] = y;|]
  }
}
var t = new C(12);
[|t.[|{| "contextRangeIndex": 2 |}x|] = 11;|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "x");
    done();
}
