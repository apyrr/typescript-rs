#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_export_as_namespace() {
    let mut t = TestingT;
    run_test_find_all_refs_export_as_namespace(&mut t);
}

fn run_test_find_all_refs_export_as_namespace(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsExportAsNamespace") {
        return;
    }
    let content = r#"// @Filename: /node_modules/a/index.d.ts
export function /*0*/f(): void;
export as namespace A;
// @Filename: /b.ts
import { /*1*/f } from "a";
// @Filename: /c.ts
A./*2*/f();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    done();
}
