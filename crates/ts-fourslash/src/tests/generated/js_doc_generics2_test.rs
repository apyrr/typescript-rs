#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_generics2() {
    let mut t = TestingT;
    run_test_js_doc_generics2(&mut t);
}

fn run_test_js_doc_generics2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocGenerics2") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
/**
 * @param {T[]} arr
 * @param {(function(T):T)} valuator
 * @template T
 */
function SortFilter(arr,valuator)
{
    return arr;
}
var a/*1*/ = SortFilter([0, 1, 2], q/*2*/ => q);
var b/*3*/ = SortFilter([0, 1, 2], undefined);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var a: number[]", "");
    f.verify_quick_info_at(t, "2", "(parameter) q: number", "");
    f.verify_quick_info_at(t, "3", "var b: number[]", "");
    done();
}
