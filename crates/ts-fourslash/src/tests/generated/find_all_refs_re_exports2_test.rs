#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_re_exports2() {
    let mut t = TestingT;
    run_test_find_all_refs_re_exports2(&mut t);
}

fn run_test_find_all_refs_re_exports2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export function /*1*/foo(): void {}
// @Filename: /b.ts
import { foo as oof } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
