#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_js_this_property_assignment() {
    let mut t = TestingT;
    run_test_find_all_refs_js_this_property_assignment(&mut t);
}

fn run_test_find_all_refs_js_this_property_assignment(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsJsThisPropertyAssignment") {
        return;
    }
    let content = r#"// @allowJs: true
// @noImplicitThis: true
// @Filename: infer.d.ts
export declare function infer(o: { m(): void } & ThisType<{ x: number }>): void;
// @Filename: a.js
import { infer } from "./infer";
infer({
    m() {
        this.x = 1;
        this./*1*/x;
    },
});
// @Filename: b.js
/**
 * @template T
 * @param {{m(): void} & ThisType<{x: number}>} o
 */
function infer(o) {}
infer({
    m() {
        this.x = 2;
        this./*2*/x;
    },
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
