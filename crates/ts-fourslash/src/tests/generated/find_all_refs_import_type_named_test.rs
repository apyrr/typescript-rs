#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_type_named() {
    let mut t = TestingT;
    run_test_find_all_refs_import_type_named(&mut t);
}

fn run_test_find_all_refs_import_type_named(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
/*1*/export type /*2*/T = number;
/*3*/export type /*4*/U = string;
// @Filename: /b.ts
const x: import("./a")./*5*/T = 0;
const x: import("./a")./*6*/U = 0;"#;
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
