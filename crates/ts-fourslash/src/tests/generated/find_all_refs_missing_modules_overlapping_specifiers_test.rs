#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_missing_modules_overlapping_specifiers() {
    let mut t = TestingT;
    run_test_find_all_refs_missing_modules_overlapping_specifiers(&mut t);
}

fn run_test_find_all_refs_missing_modules_overlapping_specifiers(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// https://github.com/microsoft/TypeScript/issues/5551
import { resolve/*0*/ as resolveUrl } from "idontcare";
import { resolve/*1*/ } from "whatever";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string()]);
    done();
}
