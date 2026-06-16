#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_default_import() {
    let mut t = TestingT;
    run_test_rename_default_import(&mut t);
}

fn run_test_rename_default_import(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameDefaultImport") {
        return;
    }
    let content = r#"// @Filename: B.ts
[|export default class /*1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}B|] {
    test() {
    }
}|]
// @Filename: A.ts
[|import /*2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}B|] from "./B";|]
let b = new [|B|]();
b.test();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[4].clone().into(),
        ],
    );
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Name("1".to_string())],
    );
    done();
}
