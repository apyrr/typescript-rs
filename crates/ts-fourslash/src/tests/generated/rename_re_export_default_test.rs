#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_re_export_default() {
    let mut t = TestingT;
    run_test_rename_re_export_default(&mut t);
}

fn run_test_rename_re_export_default(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameReExportDefault") {
        return;
    }
    let content = r#"// @Filename: /a.ts
export { default } from "./b";
[|export { default as [|{| "contextRangeIndex": 0 |}b|] } from "./b";|]
export { default as bee } from "./b";
[|import { default as [|{| "contextRangeIndex": 2 |}b|] } from "./b";|]
import { default as bee } from "./b";
[|import [|{| "contextRangeIndex": 4 |}b|] from "./b";|]
// @Filename: /b.ts
[|const [|{| "contextRangeIndex": 6 |}b|] = 0;|]
[|export default [|{| "contextRangeIndex": 8 |}b|];|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[7].clone().into(),
            f.ranges()[9].clone().into(),
        ],
    );
    done();
}
