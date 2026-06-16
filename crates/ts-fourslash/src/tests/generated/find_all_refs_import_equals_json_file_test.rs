#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_equals_json_file() {
    let mut t = TestingT;
    run_test_find_all_refs_import_equals_json_file(&mut t);
}

fn run_test_find_all_refs_import_equals_json_file(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @checkJs: true
// @resolveJsonModule: true
// @module: commonjs
// @Filename: /a.ts
import /*0*/j = require("/*1*/./j.json");
/*2*/j;
// @Filename: /b.js
const /*3*/j = require("/*4*/./j.json");
/*5*/j;
// @Filename: /j.json
/*6*/{ "x": 0 }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "2".to_string(),
            "1".to_string(),
            "4".to_string(),
            "3".to_string(),
            "5".to_string(),
            "6".to_string(),
        ],
    );
    done();
}
