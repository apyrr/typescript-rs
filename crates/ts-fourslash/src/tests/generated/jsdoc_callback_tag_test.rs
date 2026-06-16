#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_callback_tag() {
    let mut t = TestingT;
    run_test_jsdoc_callback_tag(&mut t);
}

fn run_test_jsdoc_callback_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocCallbackTag") {
        return;
    }
    let content = r#"// @lib: es5
// @strict: false
// @allowNonTsExtensions: true
// @Filename: jsdocCallbackTag.js
/**
 * @callback FooHandler - A kind of magic
 * @param {string} eventName - So many words
 * @param eventName2 {number | string} - Silence is golden
 * @param eventName3 - Osterreich mos def
 * @return {number} - DIVEKICK
 */
/**
 * @type {FooHa/*8*/ndler} callback
 */
var t/*1*/;

/**
 * @callback FooHandler2 - What, another one?
 * @param {string=} eventName - it keeps happening
 * @param {string} [eventName2] - i WARNED you dog
 */
/**
 * @type {FooH/*3*/andler2} callback
 */
var t2/*2*/;
t(/*4*/"!", /*5*/12, /*6*/false);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "1");
    f.verify_quick_info_is(t, "var t: FooHandler", "");
    f.go_to_marker(t, "2");
    f.verify_quick_info_is(t, "var t2: FooHandler2", "");
    f.go_to_marker(t, "3");
    f.verify_quick_info_is(
        t,
        "type FooHandler2 = (eventName?: string | undefined, eventName2?: string) => any",
        "- What, another one?",
    );
    f.go_to_marker(t, "8");
    f.verify_quick_info_is(t, "type FooHandler = (eventName: string, eventName2: number | string, eventName3: any) => number", "- A kind of magic");
    done();
}
