#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_of_export_equals() {
    let mut t = TestingT;
    run_test_rename_import_of_export_equals(&mut t);
}

fn run_test_rename_import_of_export_equals(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|declare namespace /*N*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}N|] {
    [|export var /*x*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}x|]: number;|]
}|]
declare module "mod" {
    [|export = [|{| "contextRangeIndex": 4 |}N|];|]
}
declare module "a" {
    [|import * as /*a*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}N|] from "mod";|]
    [|export { [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 8 |}N|] };|] // Renaming N here would rename
}
declare module "b" {
    [|import { /*b*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 10 |}N|] } from "a";|]
    export const y: typeof [|N|].[|x|];
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "N".to_string(),
            "a".to_string(),
            "b".to_string(),
            "x".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[5].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[7].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[9].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[11].clone().into(), f.ranges()[12].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[3].clone().into(), f.ranges()[13].clone().into()],
    );
    done();
}
