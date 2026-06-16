#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_js_doc_import_tag() {
    let mut t = TestingT;
    run_test_find_all_refs_js_doc_import_tag(&mut t);
}

fn run_test_find_all_refs_js_doc_import_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsJsDocImportTag") {
        return;
    }
    let content = r#"// @allowJS: true
// @checkJs: true
// @Filename: /b.ts
export interface A { }
// @Filename: /a.js
/**
 * @import { A } from "./b";
 */

/**
 * @param { [|A/**/|] } a
 */
function f(a) {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
