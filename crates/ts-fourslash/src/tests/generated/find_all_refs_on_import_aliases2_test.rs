#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_on_import_aliases2() {
    let mut t = TestingT;
    run_test_find_all_refs_on_import_aliases2(&mut t);
}

fn run_test_find_all_refs_on_import_aliases2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"//@Filename: a.ts
[|export class /*class0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}Class|] {}|]
//@Filename: b.ts
[|import { /*class1*/[|{| "contextRangeIndex": 2 |}Class|] as /*c2_0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}C2|] } from "./a";|]
var c = new /*c2_1*/[|C2|]();
//@Filename: c.ts
[|export { /*class2*/[|{| "contextRangeIndex": 6 |}Class|] as /*c3*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}C3|] } from "./a";|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "class0".to_string(),
            "class1".to_string(),
            "class2".to_string(),
            "c2_0".to_string(),
            "c2_1".to_string(),
            "c3".to_string(),
        ],
    );
    f.verify_baseline_rename_at_ranges_with_text(t, "Class");
    done();
}
