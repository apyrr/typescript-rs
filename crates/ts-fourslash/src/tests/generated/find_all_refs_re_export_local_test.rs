#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_re_export_local() {
    let mut t = TestingT;
    run_test_find_all_refs_re_export_local(&mut t);
}

fn run_test_find_all_refs_re_export_local(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noLib: true
// @strict: false
// @Filename: /a.ts
[|var /*ax0*/[|{| "isDefinition": true, "contextRangeIndex": 0 |}x|];|]
[|export { /*ax1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}x|] };|]
[|export { /*ax2*/[|{| "contextRangeIndex": 4 |}x|] as /*ay*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 4 |}y|] };|]
// @Filename: /b.ts
[|import { /*bx0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 7 |}x|], /*by0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 7 |}y|] } from "./a";|]
/*bx1*/[|x|]; /*by1*/[|y|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "ax0".to_string(),
            "ax1".to_string(),
            "ax2".to_string(),
            "bx0".to_string(),
            "bx1".to_string(),
            "ay".to_string(),
            "by0".to_string(),
            "by1".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[5].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[3].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[8].clone().into(), f.ranges()[10].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[6].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[9].clone().into(), f.ranges()[11].clone().into()],
    );
    done();
}
