#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags15() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags15(&mut t);
}

fn run_test_quick_info_js_doc_tags15(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags15") {
        return;
    }
    let content = r#"// @allowJs: true
// @checkJs: true
// @filename: /a.js
/**
 * @callback Bar
 * @param {string} name
 * @returns {string}
 */

/**
 * @typedef Foo
 * @property {Bar} getName
 */
export const foo = 1;
// @filename: /b.js
import * as _a from "./a.js";
/**
 * @implements {_a.Foo/*1*/}
 */
class C1 { }

/**
 * @extends {_a.Foo/*2*/}
 */
class C2 { }

/**
 * @augments {_a.Foo/*3*/}
 */
class C3 { }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.js");
    f.verify_baseline_hover(t, &[]);
    done();
}
