#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_dynamic_import1() {
    let mut t = TestingT;
    run_test_find_all_references_dynamic_import1(&mut t);
}

fn run_test_find_all_references_dynamic_import1(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesDynamicImport1") {
        return;
    }
    let content = r#"// @lib: es5
// @Filename: foo.ts
export function foo() { return "foo"; }
/*1*/import("/*2*/./foo")
/*3*/var x = import("/*4*/./foo")"#;
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
