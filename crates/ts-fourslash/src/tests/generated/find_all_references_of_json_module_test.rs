#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_of_json_module() {
    let mut t = TestingT;
    run_test_find_all_references_of_json_module(&mut t);
}

fn run_test_find_all_references_of_json_module(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesOfJsonModule") {
        return;
    }
    let content = r#"// @resolveJsonModule: true
// @module: commonjs
// @esModuleInterop: true
// @Filename: /foo.ts
/*1*/import /*2*/settings from "./settings.json";
/*3*/settings;
// @Filename: /settings.json
 {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
