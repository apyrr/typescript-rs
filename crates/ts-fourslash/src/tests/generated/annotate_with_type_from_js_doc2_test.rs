#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_annotate_with_type_from_js_doc2() {
    let mut t = TestingT;
    run_test_annotate_with_type_from_js_doc2(&mut t);
}

fn run_test_annotate_with_type_from_js_doc2(t: &mut TestingT) {
    if should_skip_if_failing("TestAnnotateWithTypeFromJSDoc2") {
        return;
    }
    let content = r"// @Filename: test123.ts
/** @type {number} */
var [|x|]: string;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
