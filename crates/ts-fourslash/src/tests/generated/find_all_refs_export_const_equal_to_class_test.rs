#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_export_const_equal_to_class() {
    let mut t = TestingT;
    run_test_find_all_refs_export_const_equal_to_class(&mut t);
}

fn run_test_find_all_refs_export_const_equal_to_class(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsExportConstEqualToClass") {
        return;
    }
    let content = r#"// @Filename: /a.ts
class C {}
export const /*0*/D = C;
// @Filename: /b.ts
import { /*1*/D } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string()]);
    done();
}
