#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_module_augmentation() {
    let mut t = TestingT;
    run_test_find_all_refs_module_augmentation(&mut t);
}

fn run_test_find_all_refs_module_augmentation(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/foo/index.d.ts
/*1*/export type /*2*/T = number;
// @Filename: /a.ts
import * as foo from "foo";
declare module "foo" {
    export const x: /*3*/T;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
