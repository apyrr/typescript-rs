#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_typedef_tag_navigate_to() {
    let mut t = TestingT;
    run_test_jsdoc_typedef_tag_navigate_to(&mut t);
}

fn run_test_jsdoc_typedef_tag_navigate_to(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocTypedefTagNavigateTo") {
        return;
    }
    let content = r"// @lib: es5
// @allowNonTsExtensions: true
// @Filename: jsDocTypedef_form2.js

/** @typedef {(string | number)} NumberLike */
/** @typedef {(string | number | string[])} */
var NumberLike2;

/** @type {/*1*/NumberLike} */
var numberLike;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_document_symbol(t);
    done();
}
