#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_cross_js_ts01() {
    let mut t = TestingT;
    run_test_rename_cross_js_ts01(&mut t);
}

fn run_test_rename_cross_js_ts01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @Filename: a.js
[|exports.[|{| "contextRangeIndex": 0 |}area|] = function (r) { return r * r; }|]
// @Filename: b.ts
[|import { [|{| "contextRangeIndex": 2 |}area|] } from './a';|]
var t = [|area|](10);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[4].clone().into(),
        ],
    );
    done();
}
