#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_es_module_interop_find_all_references() {
    let mut t = TestingT;
    run_test_es_module_interop_find_all_references(&mut t);
}

fn run_test_es_module_interop_find_all_references(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @esModuleInterop: true
// @Filename: /abc.d.ts
declare module "a" {
    /*1*/export const /*2*/x: number;
}
// @Filename: /b.ts
import a from "a";
a./*3*/x;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
