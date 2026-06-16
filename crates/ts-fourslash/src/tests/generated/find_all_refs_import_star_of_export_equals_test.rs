#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_star_of_export_equals() {
    let mut t = TestingT;
    run_test_find_all_refs_import_star_of_export_equals(&mut t);
}

fn run_test_find_all_refs_import_star_of_export_equals(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsImportStarOfExportEquals") {
        return;
    }
    let content = r#"// @allowSyntheticDefaultimports: true
// @Filename: /node_modules/a/index.d.ts
[|declare function /*a0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}a|](): void;|]
[|declare namespace /*a1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}a|] {
    export const x: number;
}|]
[|export = /*a2*/[|{| "contextRangeIndex": 4 |}a|];|]
// @Filename: /b.ts
[|import /*b0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}b|] from "a";|]
/*b1*/[|b|]();
[|b|].x;
// @Filename: /c.ts
[|import /*c0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 10 |}a|] from "a";|]
/*c1*/[|a|]();
/*c2*/[|a|].x;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "a0".to_string(),
            "a1".to_string(),
            "a2".to_string(),
            "b0".to_string(),
            "b1".to_string(),
            "c0".to_string(),
            "c1".to_string(),
            "c2".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[7].clone().into(),
            f.ranges()[8].clone().into(),
            f.ranges()[9].clone().into(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[11].clone().into(),
            f.ranges()[12].clone().into(),
            f.ranges()[13].clone().into(),
        ],
    );
    done();
}
