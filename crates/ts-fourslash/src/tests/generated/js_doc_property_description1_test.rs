#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description1() {
    let mut t = TestingT;
    run_test_js_doc_property_description1(&mut t);
}

fn run_test_js_doc_property_description1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription1") {
        return;
    }
    let content = r"interface StringExample {
    /** Something generic */
    [p: string]: any; 
    /** Something specific */
    property: number;
}
function stringExample(e: StringExample) {
    console.log(e./*property*/property);
    console.log(e./*string*/anything); 
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "property",
        "(property) StringExample.property: number",
        "Something specific",
    );
    f.verify_quick_info_at(
        t,
        "string",
        "(index) StringExample[string]: any",
        "Something generic",
    );
    done();
}
