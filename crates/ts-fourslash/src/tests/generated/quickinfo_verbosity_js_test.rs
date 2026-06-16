#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_js() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_js(&mut t);
}

fn run_test_quickinfo_verbosity_js(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityJs") {
        return;
    }
    let content = r"// @Filename: somefile.js
// @allowJs: true
/**
 * @typedef {Object} SomeType
 * @property {string} prop1
 */
/** @type {SomeType} */
const a/*1*/ = {
    prop1: 'value',
}
/**
 * @typedef {Object} SomeType2/*2*/
 * @property {number} prop2
 * @property {SomeType} prop3
 */
/** @type {SomeType[]} */
const ss = [{ prop1: 'value' }, { prop1: 'value' }];
const d = ss.map((s/*3*/) => s.prop1);
/** @param {SomeType} a
 * @returns {SomeType}
 */
function someFun/*4*/(a) {
    return a;
}
someFun.what = 'what';
class SomeClass/*5*/ {
    /** @type {SomeType2} */
    b;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("1".to_string(), vec![0, 1]),
            ("2".to_string(), vec![0, 1]),
            ("3".to_string(), vec![0, 1]),
            ("4".to_string(), vec![0, 1]),
            ("5".to_string(), vec![0, 1, 2]),
        ]),
    );
    done();
}
