#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description10() {
    let mut t = TestingT;
    run_test_js_doc_property_description10(&mut t);
}

fn run_test_js_doc_property_description10(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription10") {
        return;
    }
    let content = r"class MultipleClass {
    /** Something generic */
    [key: number | symbol | `data-${string}` | `data-${number}`]: string;
}
function multipleClass(e: typeof MultipleClass) {
    console.log(e./*multipleClass*/anything);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "multipleClass", "any", "");
    done();
}
