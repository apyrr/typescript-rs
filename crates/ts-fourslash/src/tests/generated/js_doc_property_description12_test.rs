#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description12() {
    let mut t = TestingT;
    run_test_js_doc_property_description12(&mut t);
}

fn run_test_js_doc_property_description12(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription12") {
        return;
    }
    let content = r"type SymbolAlias = {
    /** Something generic */
    [p: symbol]: string;
}
function symbolAlias(e: SymbolAlias) {
    console.log(e./*symbolAlias*/anything);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "symbolAlias", "any", "");
    done();
}
