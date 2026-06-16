#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_comments_line_preservation() {
    let mut t = TestingT;
    run_test_comments_line_preservation(&mut t);
}

fn run_test_comments_line_preservation(t: &mut TestingT) {
    if should_skip_if_failing("TestCommentsLinePreservation") {
        return;
    }
    let content = r#"/** This is firstLine
  * This is second Line
  * 
  * This is fourth Line
  */
var /*a*/a: string;
/** 
  * This is firstLine
  * This is second Line
  * 
  * This is fourth Line
  */
var /*b*/b: string;
/** 
  * This is firstLine
  * This is second Line
  * 
  * This is fourth Line
  *
  */
var /*c*/c: string;
/** 
  * This is firstLine
  * This is second Line
  * @param param
  * @random tag This should be third line
  */
function /*d*/d(param: string) { /*1*/param = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param
  */
function /*e*/e(param: string) { /*2*/param = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param1 first line of param
  *
  *  param information third line
  * @random tag This should be third line
  */
function /*f*/f(param1: string) { /*3*/param1 = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param1
  *
  *  param information first line
  * @random tag This should be third line
  */
function /*g*/g(param1: string) { /*4*/param1 = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param1
  *
  *  param information first line
  *
  *  param information third line
  * @random tag This should be third line
  */
function /*h*/h(param1: string) { /*5*/param1 = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param1
  *
  *  param information first line
  *
  *  param information third line
  *
  */
function /*i*/i(param1: string) { /*6*/param1 = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param1
  *
  *  param information first line
  *
  *  param information third line
  */
function /*j*/j(param1: string) { /*7*/param1 = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param1 hello   @randomtag 
  *
  *  random information first line
  *
  *  random information third line
  */
function /*k*/k(param1: string) { /*8*/param1 = "hello"; }
/** 
  * This is firstLine
  * This is second Line
  * @param param1 first Line text
  *
  * @param param1 
  *
  * blank line that shouldnt be shown when starting this 
  * second time information about the param again
  */
function /*l*/l(param1: string) { /*9*/param1 = "hello"; }
     /** 
       * This is firstLine
 This is second Line
 [1]: third * line
 @param param1 first Line text
 second line text
 */
function /*m*/m(param1: string) { /*10*/param1 = "hello"; }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "a",
        "var a: string",
        "This is firstLine\nThis is second Line\n\nThis is fourth Line",
    );
    f.verify_quick_info_at(
        t,
        "b",
        "var b: string",
        "This is firstLine\nThis is second Line\n\nThis is fourth Line",
    );
    f.verify_quick_info_at(
        t,
        "c",
        "var c: string",
        "This is firstLine\nThis is second Line\n\nThis is fourth Line",
    );
    f.verify_quick_info_at(
        t,
        "d",
        "function d(param: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(t, "1", "(parameter) param: string", "");
    f.verify_quick_info_at(
        t,
        "e",
        "function e(param: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(t, "2", "(parameter) param: string", "");
    f.verify_quick_info_at(
        t,
        "f",
        "function f(param1: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(
        t,
        "3",
        "(parameter) param1: string",
        "first line of param\n\nparam information third line",
    );
    f.verify_quick_info_at(
        t,
        "g",
        "function g(param1: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(
        t,
        "4",
        "(parameter) param1: string",
        " param information first line",
    );
    f.verify_quick_info_at(
        t,
        "h",
        "function h(param1: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(
        t,
        "5",
        "(parameter) param1: string",
        " param information first line\n\n param information third line",
    );
    f.verify_quick_info_at(
        t,
        "i",
        "function i(param1: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(
        t,
        "6",
        "(parameter) param1: string",
        " param information first line\n\n param information third line",
    );
    f.verify_quick_info_at(
        t,
        "j",
        "function j(param1: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(
        t,
        "7",
        "(parameter) param1: string",
        " param information first line\n\n param information third line",
    );
    f.verify_quick_info_at(
        t,
        "k",
        "function k(param1: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(t, "8", "(parameter) param1: string", "hello");
    f.verify_quick_info_at(
        t,
        "l",
        "function l(param1: string): void",
        "This is firstLine\nThis is second Line",
    );
    f.verify_quick_info_at(t, "9", "(parameter) param1: string", "first Line text\nblank line that shouldnt be shown when starting this \nsecond time information about the param again");
    f.verify_quick_info_at(
        t,
        "m",
        "function m(param1: string): void",
        "This is firstLine\nThis is second Line\n[1]: third * line",
    );
    f.verify_quick_info_at(
        t,
        "10",
        "(parameter) param1: string",
        "first Line text\nsecond line text",
    );
    done();
}
