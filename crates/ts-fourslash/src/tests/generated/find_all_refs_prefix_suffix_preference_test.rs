#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_prefix_suffix_preference() {
    let mut t = TestingT;
    run_test_find_all_refs_prefix_suffix_preference(&mut t);
}

fn run_test_find_all_refs_prefix_suffix_preference(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /file1.ts
declare function log(s: string | number): void;
[|const /*q0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}q|] = 1;|]
[|export { /*q1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}q|] };|]
const x = {
    [|/*z0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 4 |}z|]: 'value'|]
}
[|const { /*z1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}z|] } = x;|]
log(/*z2*/[|z|]);
// @Filename: /file2.ts
declare function log(s: string | number): void;
[|import { /*q2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 9 |}q|] } from "./file1";|]
log(/*q3*/[|q|] + 1);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "q0".to_string(),
            "q1".to_string(),
            "q2".to_string(),
            "q3".to_string(),
            "z0".to_string(),
            "z1".to_string(),
            "z2".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[10].clone().into(),
            f.ranges()[11].clone().into(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[10].clone().into(),
            f.ranges()[11].clone().into(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[5].clone().into(),
            f.ranges()[7].clone().into(),
            f.ranges()[8].clone().into(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[5].clone().into(),
            f.ranges()[7].clone().into(),
            f.ranges()[8].clone().into(),
        ],
    );
    done();
}
