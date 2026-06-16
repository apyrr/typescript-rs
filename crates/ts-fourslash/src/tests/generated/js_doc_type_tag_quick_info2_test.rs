#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_type_tag_quick_info2() {
    let mut t = TestingT;
    run_test_js_doc_type_tag_quick_info2(&mut t);
}

fn run_test_js_doc_type_tag_quick_info2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocTypeTagQuickInfo2") {
        return;
    }
    let content = r"// @lib: es5
// @strict: true
// @allowJs: true
// @Filename: jsDocTypeTag2.js
/** @type {string} */
var /*1*/s;
/** @type {number} */
var /*2*/n;
/** @type {boolean} */
var /*3*/b;
/** @type {void} */
var /*4*/v;
/** @type {undefined} */
var /*5*/u;
/** @type {null} */
var /*6*/nl;
/** @type {array} */
var /*7*/a;
/** @type {promise} */
var /*8*/p;
/** @type {?number} */
var /*9*/nullable;
/** @type {function} */
var /*10*/func;
/** @type {function (number): number} */
var /*11*/func1;
/** @type {string | number} */
var /*12*/sOrn;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
