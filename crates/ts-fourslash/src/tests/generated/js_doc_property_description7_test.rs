#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description7() {
    let mut t = TestingT;
    run_test_js_doc_property_description7(&mut t);
}

fn run_test_js_doc_property_description7(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription7") {
        return;
    }
    let content = r"class StringClass {
    /** Something generic */
    static [p: string]: any;
}
function stringClass(e: typeof StringClass) {
    console.log(e./*stringClass*/anything);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "stringClass",
        "(index) StringClass[string]: any",
        "Something generic",
    );
    done();
}
