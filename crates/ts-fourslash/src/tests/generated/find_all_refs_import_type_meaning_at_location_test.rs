#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_type_meaning_at_location() {
    let mut t = TestingT;
    run_test_find_all_refs_import_type_meaning_at_location(&mut t);
}

fn run_test_find_all_refs_import_type_meaning_at_location(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefs_importType_meaningAtLocation") {
        return;
    }
    let content = r#"// @Filename: /a.ts
/*1*/export type /*2*/T = 0;
/*3*/export const /*4*/T = 0;
// @Filename: /b.ts
const x: import("./a")./*5*/T = 0;
const x: typeof import("./a")./*6*/T = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
        ],
    );
    done();
}
