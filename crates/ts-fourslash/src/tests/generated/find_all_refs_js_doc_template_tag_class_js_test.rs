#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_js_doc_template_tag_class_js() {
    let mut t = TestingT;
    run_test_find_all_refs_js_doc_template_tag_class_js(&mut t);
}

fn run_test_find_all_refs_js_doc_template_tag_class_js(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsJsDocTemplateTag_class_js") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
/** @template /*1*/T */
class C {
    constructor() {
        /** @type {/*2*/T} */
        this.x = null;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
