#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_and_export() {
    let mut t = TestingT;
    run_test_rename_import_and_export(&mut t);
}

fn run_test_rename_import_and_export(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameImportAndExport") {
        return;
    }
    let content = r#"[|import [|{| "contextRangeIndex": 0 |}a|] from "module";|]
[|export { [|{| "contextRangeIndex": 2 |}a|] };|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[3].clone().into()],
    );
    done();
}
