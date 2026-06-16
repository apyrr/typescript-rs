#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description4() {
    let mut t = TestingT;
    run_test_js_doc_property_description4(&mut t);
}

fn run_test_js_doc_property_description4(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription4") {
        return;
    }
    let content = r"interface MultipleExample {
    /** Something generic */
    [key: string | number | symbol]: string;
}
function multipleExample(e: MultipleExample) {
    console.log(e./*multiple*/anything);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "multiple",
        "(index) MultipleExample[string | number | symbol]: string",
        "Something generic",
    );
    done();
}
