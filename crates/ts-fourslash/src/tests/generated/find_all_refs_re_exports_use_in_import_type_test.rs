#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_re_exports_use_in_import_type() {
    let mut t = TestingT;
    run_test_find_all_refs_re_exports_use_in_import_type(&mut t);
}

fn run_test_find_all_refs_re_exports_use_in_import_type(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsReExportsUseInImportType") {
        return;
    }
    let content = r#"// @Filename: /foo/types/types.ts
[|export type /*full0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}Full|] = { prop: string; };|]
// @Filename: /foo/types/index.ts
[|import * as /*foo0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 2 |}foo|] from './types';|]
[|export { /*foo1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 4 |}foo|] };|]
// @Filename: /app.ts
[|import { /*foo2*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 6 |}foo|] } from './foo/types';|]
export type fullType = /*foo3*/[|foo|]./*full1*/[|Full|];
type namespaceImport = typeof import('./foo/types');
type fullType2 = import('./foo/types')./*foo4*/[|foo|]./*full2*/[|Full|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "full0".to_string(),
            "full1".to_string(),
            "full2".to_string(),
            "foo0".to_string(),
            "foo1".to_string(),
            "foo2".to_string(),
            "foo3".to_string(),
            "foo4".to_string(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[9].clone().into(),
            f.ranges()[11].clone().into(),
        ],
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[3].clone().into()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[5].clone().into(), f.ranges()[10].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[7].clone().into(), f.ranges()[8].clone().into()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[7].clone().into(),
            f.ranges()[8].clone().into(),
            f.ranges()[10].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
        ],
    );
    done();
}
