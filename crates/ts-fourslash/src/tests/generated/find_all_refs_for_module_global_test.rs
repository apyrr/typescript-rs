#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_module_global() {
    let mut t = TestingT;
    run_test_find_all_refs_for_module_global(&mut t);
}

fn run_test_find_all_refs_for_module_global(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/foo/index.d.ts
export const x = 0;
// @Filename: /b.ts
/// <reference types="foo" />
import { x } from "/*1*/foo";
declare module "foo" {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
