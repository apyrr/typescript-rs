#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_type_typeof_import() {
    let mut t = TestingT;
    run_test_find_all_refs_import_type_typeof_import(&mut t);
}

fn run_test_find_all_refs_import_type_typeof_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export const x = 0;
// @Filename: /b.ts
/*1*/const x: typeof import("/*2*/./a") = { x: 0 };
/*3*/const y: typeof import("/*4*/./a") = { x: 0 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
