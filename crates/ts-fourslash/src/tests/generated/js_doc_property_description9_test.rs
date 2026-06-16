#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description9() {
    let mut t = TestingT;
    run_test_js_doc_property_description9(&mut t);
}

fn run_test_js_doc_property_description9(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription9") {
        return;
    }
    let content = r"class LiteralClass {
    /** Something generic */
    static [key: `prefix${string}`]: any;
    /** Something else */
    static [key: `prefix${number}`]: number;
}
function literalClass(e: typeof LiteralClass) {
    console.log(e./*literal1Class*/prefixMember); 
    console.log(e./*literal2Class*/anything);
    console.log(e./*literal3Class*/prefix0);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "literal1Class",
        "(index) LiteralClass[`prefix${string}`]: any",
        "Something generic",
    );
    f.verify_quick_info_at(t, "literal2Class", "any", "");
    f.verify_quick_info_at(
        t,
        "literal3Class",
        "(index) LiteralClass[`prefix${string}` | `prefix${number}`]: any",
        "Something generic\nSomething else",
    );
    done();
}
