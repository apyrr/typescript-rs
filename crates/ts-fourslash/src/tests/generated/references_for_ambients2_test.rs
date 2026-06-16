#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_ambients2() {
    let mut t = TestingT;
    run_test_references_for_ambients2(&mut t);
}

fn run_test_references_for_ambients2(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForAmbients2") {
        return;
    }
    let content = r#"// @Filename: /defA.ts
declare module "a" {
    /*1*/export type /*2*/T = number;
}
// @Filename: /defB.ts
declare module "b" {
    export import a = require("a");
    export const x: a./*3*/T;
}
// @Filename: /defC.ts
declare module "c" {
    import b = require("b");
    const x: b.a./*4*/T;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
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
