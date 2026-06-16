#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_re_export_star() {
    let mut t = TestingT;
    run_test_find_all_refs_re_export_star(&mut t);
}

fn run_test_find_all_refs_re_export_star(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsReExportStar") {
        return;
    }
    let content = r#"// @Filename: /a.ts
export function /*0*/foo(): void {}
// @Filename: /b.ts
export * from "./a";
// @Filename: /c.ts
import { /*1*/foo } from "./b";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string()]);
    done();
}
