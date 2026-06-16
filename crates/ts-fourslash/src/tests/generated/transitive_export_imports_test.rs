#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_transitive_export_imports() {
    let mut t = TestingT;
    run_test_transitive_export_imports(&mut t);
}

fn run_test_transitive_export_imports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: commonjs
// @Filename: a.ts
[|class /*1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}A|] {
}|]
[|export = [|{| "contextRangeIndex": 2 |}A|];|]
// @Filename: b.ts
[|export import /*2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 4 |}b|] = require('./a');|]
// @Filename: c.ts
[|import /*3*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}b|] = require('./b');|]
var a = new /*4*/[|b|]./**/[|b|]();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[3].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[5].clone().into(), f.ranges()[9].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[7].clone().into(), f.ranges()[8].clone().into()],
    );
    done();
}
