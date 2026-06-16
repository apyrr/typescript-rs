#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_and_export_in_diff_files() {
    let mut t = TestingT;
    run_test_rename_import_and_export_in_diff_files(&mut t);
}

fn run_test_rename_import_and_export_in_diff_files(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: a.ts
[|export var /*1*/[|{| "isDefinition": true, "contextRangeIndex": 0 |}a|];|]
// @Filename: b.ts
[|import { /*2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}a|] } from './a';|]
[|export { /*3*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 4 |}a|] };|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
        ],
    );
    done();
}
