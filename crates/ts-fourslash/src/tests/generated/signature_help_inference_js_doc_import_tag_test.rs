#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_inference_js_doc_import_tag() {
    let mut t = TestingT;
    run_test_signature_help_inference_js_doc_import_tag(&mut t);
}

fn run_test_signature_help_inference_js_doc_import_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpInferenceJsDocImportTag") {
        return;
    }
    let content = r"// @allowJS: true
// @checkJs: true
// @module: esnext
// @filename: a.ts
export interface Foo {}
// @filename: b.js
/**
 * @import {
 *     Foo
 * } from './a'
 */

/**
 * @param {Foo} a
 */
function foo(a) {}
foo(/**/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
