#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_text_formatting1() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_text_formatting1(&mut t);
}

fn run_test_quick_info_js_doc_text_formatting1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTextFormatting1") {
        return;
    }
    let content = r"/**
 * @param {number} var1 **Highlighted text**
 * @param {string} var2 Another **Highlighted text**
*/
function f1(var1, var2) { }

/**
 * @param {number} var1 *Regular text with an asterisk
 * @param {string} var2 Another *Regular text with an asterisk
*/
function f2(var1, var2) { }

/**
 * @param {number} var1 
 * *Regular text with an asterisk
 * @param {string} var2 
 * Another *Regular text with an asterisk
*/
function f3(var1, var2) { }

/**
 * @param {number} var1 
 * **Highlighted text**
 * @param {string} var2 
 * Another **Highlighted text**
*/
function f4(var1, var2) { }

/**
 * @param {number} var1 
   **Highlighted text**
 * @param {string} var2 
   Another **Highlighted text**
*/
function f5(var1, var2) { }

f1(/*1*/);
f2(/*2*/);
f3(/*3*/);
f4(/*4*/);
f5(/*5*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
