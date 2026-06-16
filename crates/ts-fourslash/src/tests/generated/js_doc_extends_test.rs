#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_extends() {
    let mut t = TestingT;
    run_test_js_doc_extends(&mut t);
}

fn run_test_js_doc_extends(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocExtends") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: dummy.js
/**
 * @extends {Thing<string>}
 */
class MyStringThing extends Thing {
    constructor() {
        var x = this.mine;
        x/**/;
    }
}
// @Filename: declarations.d.ts
declare class Thing<T> {
    mine: T;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "(local var) x: string", "");
    done();
}
