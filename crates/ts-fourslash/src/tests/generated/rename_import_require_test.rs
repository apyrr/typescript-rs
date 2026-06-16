#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_require() {
    let mut t = TestingT;
    run_test_rename_import_require(&mut t);
}

fn run_test_rename_import_require(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameImportRequire") {
        return;
    }
    let content = r#"// @Filename: /a.ts
[|import [|{| "contextRangeIndex": 0 |}e|] = require("mod4");|]
[|e|];
a = { [|e|] };
[|export { [|{| "contextRangeIndex": 4 |}e|] };|]
// @Filename: /b.ts
[|import { [|{| "contextRangeIndex": 6 |}e|] } from "./a";|]
[|export { [|{| "contextRangeIndex": 8 |}e|] };|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[2].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[7].clone().into(),
            f.ranges()[9].clone().into(),
        ],
    );
    done();
}
