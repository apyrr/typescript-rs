#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_transitive_export_imports2() {
    let mut t = TestingT;
    run_test_transitive_export_imports2(&mut t);
}

fn run_test_transitive_export_imports2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: a.ts
[|namespace /*A*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}A|] {
    export const x = 0;
}|]
// @Filename: b.ts
[|export import /*B*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}B|] = [|A|];|]
[|B|].x;
// @Filename: c.ts
[|import { /*C*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}B|] } from "./b";|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["A".to_string(), "B".to_string(), "C".to_string()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[4].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[3].clone().into(), f.ranges()[5].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[7].clone().into()]);
    done();
}
