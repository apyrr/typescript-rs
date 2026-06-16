#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_transitive_export_imports3() {
    let mut t = TestingT;
    run_test_transitive_export_imports3(&mut t);
}

fn run_test_transitive_export_imports3(t: &mut TestingT) {
    if should_skip_if_failing("TestTransitiveExportImports3") {
        return;
    }
    let content = r#"// @Filename: a.ts
[|export function /*f*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}f|]() {}|]
// @Filename: b.ts
[|export { [|{| "contextRangeIndex": 2 |}f|] as /*g0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}g|] } from "./a";|]
[|import { /*f2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 5 |}f|] } from "./a";|]
[|import { /*g1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 7 |}g|] } from "./b";|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "f".to_string(),
            "g0".to_string(),
            "g1".to_string(),
            "f2".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[3].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[6].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[4].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[8].clone().into()]);
    done();
}
