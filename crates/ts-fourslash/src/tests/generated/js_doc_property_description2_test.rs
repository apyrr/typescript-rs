#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description2() {
    let mut t = TestingT;
    run_test_js_doc_property_description2(&mut t);
}

fn run_test_js_doc_property_description2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription2") {
        return;
    }
    let content = r"interface SymbolExample {
    /** Something generic */
    [key: symbol]: string;
}
function symbolExample(e: SymbolExample) {
    console.log(e./*symbol*/anything);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "symbol", "any", "");
    done();
}
