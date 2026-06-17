#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_js_doc_import1() {
    let mut t = TestingT;
    run_test_auto_import_js_doc_import1(&mut t);
}

fn run_test_auto_import_js_doc_import1(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportJsDocImport1") {
        return;
    }
    let content = r#"// @verbatimModuleSyntax: true
// @target: esnext
// @allowJs: true
// @checkJs: true
// @Filename: /foo.ts
 export const A = 1;
 export type B = { x: number };
 export type C = 1;
 export class D { y: string }
// @Filename: /test.js
/**
 * @import { A, D, C } from "./foo"
 */

/**
 * @param { typeof A } a
 * @param { B/**/ | C } b
 * @param { C } c
 * @param { D } d
 */
export function f(a, b, c, d) { }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"/**
 * @import { A, D, C, B } from "./foo"
 */

/**
 * @param { typeof A } a
 * @param { B | C } b
 * @param { C } c
 * @param { D } d
 */
export function f(a, b, c, d) { }"#
            .to_string()],
        None,
    );
    done();
}
