#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_rename_import_with_same_name() {
    let mut t = TestingT;
    run_test_find_all_refs_rename_import_with_same_name(&mut t);
}

fn run_test_find_all_refs_rename_import_with_same_name(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsRenameImportWithSameName") {
        return;
    }
    let content = r#"// @Filename: /a.ts
[|export const /*0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}x|] = 0;|]
//@Filename: /b.ts
[|import { /*1*/[|{| "contextRangeIndex": 2 |}x|] as /*2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}x|] } from "./a";|]
/*3*/[|x|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[4].clone().into(),
            f.ranges()[5].clone().into(),
        ],
    );
    done();
}
