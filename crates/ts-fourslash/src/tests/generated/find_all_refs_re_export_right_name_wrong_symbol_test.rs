#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_re_export_right_name_wrong_symbol() {
    let mut t = TestingT;
    run_test_find_all_refs_re_export_right_name_wrong_symbol(&mut t);
}

fn run_test_find_all_refs_re_export_right_name_wrong_symbol(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsReExportRightNameWrongSymbol") {
        return;
    }
    let content = r#"// @Filename: /a.ts
[|export const /*a*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}x|] = 0;|]
// @Filename: /b.ts
[|export const /*b*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}x|] = 0;|]
//@Filename: /c.ts
[|export { /*cFromB*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 4 |}x|] } from "./b";|]
[|import { /*cFromA*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}x|] } from "./a";|]
/*cUse*/[|x|];
// @Filename: /d.ts
[|import { /*d*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 9 |}x|] } from "./c";|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "a".to_string(),
            "b".to_string(),
            "cFromB".to_string(),
            "cFromA".to_string(),
            "cUse".to_string(),
            "d".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[1].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[7].clone().into(), f.ranges()[8].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[3].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[5].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[10].clone().into()]);
    done();
}
