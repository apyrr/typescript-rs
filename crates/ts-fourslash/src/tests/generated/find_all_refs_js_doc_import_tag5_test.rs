#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_js_doc_import_tag5() {
    let mut t = TestingT;
    run_test_find_all_refs_js_doc_import_tag5(&mut t);
}

fn run_test_find_all_refs_js_doc_import_tag5(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsJsDocImportTag5") {
        return;
    }
    let content = r#"// @checkJs: true
// @Filename: /a.js
export default function /*0*/a() {}
// @Filename: /b.js
/** @import /*1*/a, * as ns from "./a" */"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string()]);
    done();
}
