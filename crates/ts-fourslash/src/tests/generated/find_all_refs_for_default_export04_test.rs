#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_default_export04() {
    let mut t = TestingT;
    run_test_find_all_refs_for_default_export04(&mut t);
}

fn run_test_find_all_refs_for_default_export04(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForDefaultExport04") {
        return;
    }
    let content = r#"// @Filename: /a.ts
const /*0*/a = 0;
export /*1*/default /*2*/a;
// @Filename: /b.ts
import /*3*/a from "./a";
/*4*/a;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "2".to_string(),
            "1".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
