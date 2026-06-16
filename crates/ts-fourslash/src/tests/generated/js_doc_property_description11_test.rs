#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description11() {
    let mut t = TestingT;
    run_test_js_doc_property_description11(&mut t);
}

fn run_test_js_doc_property_description11(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription11") {
        return;
    }
    let content = r"type AliasExample = {
    /** Something generic */
    [p: string]: string;
    /** Something else */
    [key: `any${string}`]: string;
}
function aliasExample(e: AliasExample) {
    console.log(e./*alias*/anything);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "alias",
        "(index) AliasExample[string | `any${string}`]: string",
        "Something generic\nSomething else",
    );
    done();
}
