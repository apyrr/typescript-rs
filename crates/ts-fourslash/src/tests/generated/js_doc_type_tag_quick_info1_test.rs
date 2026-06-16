#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_type_tag_quick_info1() {
    let mut t = TestingT;
    run_test_js_doc_type_tag_quick_info1(&mut t);
}

fn run_test_js_doc_type_tag_quick_info1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocTypeTagQuickInfo1") {
        return;
    }
    let content = r"// @lib: es5
// @strict: true
// @allowJs: true
// @Filename: jsDocTypeTag1.js
/** @type {String} */
var /*1*/S;
/** @type {Number} */
var /*2*/N;
/** @type {Boolean} */
var /*3*/B;
/** @type {Void} */
var /*4*/V;
/** @type {Undefined} */
var /*5*/U;
/** @type {Null} */
var /*6*/Nl;
/** @type {Array} */
var /*7*/A;
/** @type {Promise} */
var /*8*/P;
/** @type {Object} */
var /*9*/Obj;
/** @type {Function} */
var /*10*/Func;
/** @type {*} */
var /*11*/AnyType;
/** @type {?} */
var /*12*/QType;
/** @type {String|Number} */
var /*13*/SOrN;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
