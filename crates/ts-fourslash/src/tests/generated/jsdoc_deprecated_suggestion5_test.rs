#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_deprecated_suggestion5() {
    let mut t = TestingT;
    run_test_jsdoc_deprecated_suggestion5(&mut t);
}

fn run_test_jsdoc_deprecated_suggestion5(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocDeprecated_suggestion5") {
        return;
    }
    let content = r#"// @checkJs: true
// @allowJs: true
// @Filename: jsdocDeprecated_suggestion5.js
/** @typedef {{ email: string, nickName?: string }} U2 */
/** @type {U2} */
const u2 = { email: "" }
/**
 * @callback K
 * @param {any} ctx
 * @return {void}
 */
/** @type {K} */
const cc = _k => {}
/** @enum {number} */
const DOOM = { e: 1, m: 1 }
/** @type {DOOM} */
const kneeDeep = DOOM.e"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
