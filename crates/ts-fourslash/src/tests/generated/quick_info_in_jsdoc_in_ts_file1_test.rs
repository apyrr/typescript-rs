#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_in_jsdoc_in_ts_file1() {
    let mut t = TestingT;
    run_test_quick_info_in_jsdoc_in_ts_file1(&mut t);
}

fn run_test_quick_info_in_jsdoc_in_ts_file1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInJsdocInTsFile1") {
        return;
    }
    let content = r"/** @type {() => { /*1*/data: string[] }} */
function test(): { data: string[] } {
  return {
    data: [],
  };
}

/** @returns {{ /*2*/data: string[] }} */
function test2(): { data: string[] } {
  return {
    data: [],
  };
}

/** @type {{ /*3*/bar: string; }} */
const test3 = { bar: '' };

type SomeObj = { bar: string; };
/** @type {SomeObj/*4*/} */
const test4 = { bar: '' }

/**
 * @param/*5*/ stuff/*6*/ Stuff to do stuff with
 */
function doStuffWithStuff(stuff: { quantity: number }) {}

declare const stuff: { quantity: number };
/** @see {doStuffWithStuff/*7*/} */
if (stuff.quantity) {}

/** @type {(a/*8*/: string) => void} */
function test2(a: string) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "", "");
    f.verify_quick_info_at(t, "2", "", "");
    f.verify_quick_info_at(t, "3", "", "");
    f.verify_quick_info_at(t, "4", "type SomeObj = {\n    bar: string;\n}", "");
    f.verify_quick_info_at(
        t,
        "5",
        "(parameter) stuff: {\n    quantity: number;\n}",
        "Stuff to do stuff with",
    );
    f.verify_quick_info_at(
        t,
        "6",
        "(parameter) stuff: {\n    quantity: number;\n}",
        "Stuff to do stuff with",
    );
    f.verify_quick_info_at(
        t,
        "7",
        "function doStuffWithStuff(stuff: {\n    quantity: number;\n}): void",
        "",
    );
    f.verify_quick_info_at(t, "8", "", "");
    done();
}
