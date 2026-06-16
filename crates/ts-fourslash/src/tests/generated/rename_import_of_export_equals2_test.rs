#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_of_export_equals2() {
    let mut t = TestingT;
    run_test_rename_import_of_export_equals2(&mut t);
}

fn run_test_rename_import_of_export_equals2(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameImportOfExportEquals2") {
        return;
    }
    let content = r#"[|declare namespace /*N*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}N|] {
    export var x: number;
}|]
declare module "mod" {
    [|export = [|{| "contextRangeIndex": 2 |}N|];|]
}
declare module "a" {
    [|import * as /*O*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 4 |}O|] from "mod";|]
    [|export { [|{| "contextRangeIndex": 6 |}O|] as /*P*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}P|] };|] // Renaming N here would rename
}
declare module "b" {
    [|import { [|{| "contextRangeIndex": 9 |}P|] as /*Q*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 9 |}Q|] } from "a";|]
    export const y: typeof [|Q|].x;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "N".to_string(),
            "O".to_string(),
            "P".to_string(),
            "Q".to_string(),
        ],
    );
    f.verify_baseline_rename_at_ranges_with_text(t, "N");
    done();
}
