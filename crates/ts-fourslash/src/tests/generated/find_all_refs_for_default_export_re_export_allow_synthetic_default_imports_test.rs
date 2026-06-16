#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_default_export_re_export_allow_synthetic_default_imports() {
    let mut t = TestingT;
    run_test_find_all_refs_for_default_export_re_export_allow_synthetic_default_imports(&mut t);
}

fn run_test_find_all_refs_for_default_export_re_export_allow_synthetic_default_imports(
    t: &mut TestingT,
) {
    if should_skip_if_failing(
        "TestFindAllRefsForDefaultExport_reExport_allowSyntheticDefaultImports",
    ) {
        return;
    }
    let content = r#"// @allowSyntheticDefaultImports: true
// @module: commonjs
// @Filename: /export.ts
const /*0*/foo = 1;
export = /*1*/foo;
// @Filename: /re-export.ts
export { /*2*/default } from "./export";
// @Filename: /re-export-dep.ts
import /*3*/fooDefault from "./re-export";"#;
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
    done();
}
