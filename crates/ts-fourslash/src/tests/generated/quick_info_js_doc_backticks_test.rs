#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_backticks() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_backticks(&mut t);
}

fn run_test_quick_info_js_doc_backticks(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJSDocBackticks") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @checkJs: true
// @strict: true
// @Filename: jsdocParseMatchingBackticks.js
/**
 * `@param` initial at-param is OK in title comment
 * @param {string} x hi there `@param`
 * @param {string} y hi there `@ * param
 *                   this is the margin
 */
export function f(x, y) {
    return x/*x*/ + y/*y*/
}
f/*f*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "f");
    f.verify_quick_info_is(
        t,
        "function f(x: string, y: string): string",
        "`@param` initial at-param is OK in title comment",
    );
    f.go_to_marker(t, "x");
    f.verify_quick_info_is(t, "(parameter) x: string", "hi there `@param`");
    f.go_to_marker(t, "y");
    f.verify_quick_info_is(
        t,
        "(parameter) y: string",
        "hi there `@ * param\nthis is the margin",
    );
    done();
}
