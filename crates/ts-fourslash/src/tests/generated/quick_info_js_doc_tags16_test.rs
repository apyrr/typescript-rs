#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags16() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags16(&mut t);
}

fn run_test_quick_info_js_doc_tags16(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags16") {
        return;
    }
    let content = r"class A {
    /**
     * Description text here.
     *
     * @virtual
     */
    foo() { }
}

class B extends A {
    override /*1*/foo() { }
}

class C extends B {
    override /*2*/foo() { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
