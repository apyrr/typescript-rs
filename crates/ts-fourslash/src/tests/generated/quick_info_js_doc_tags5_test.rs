#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags5() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags5(&mut t);
}

fn run_test_quick_info_js_doc_tags5(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags5") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @Filename: quickInfoJsDocTags5.js
class Foo {
    /**
     * comment
     * @author Me <me@domain.tld>
     * @see x (the parameter)
     * @param {number} x - x comment
     * @param {number} y - y comment
     * @returns The result
     */
    method(x, y) {
       return x + y;
    }
}

class Bar extends Foo {
    /**/method(x, y) {
        const res = super.method(x, y) + 100;
        return res;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
