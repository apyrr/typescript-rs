#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_property_description6() {
    let mut t = TestingT;
    run_test_js_doc_property_description6(&mut t);
}

fn run_test_js_doc_property_description6(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocPropertyDescription6") {
        return;
    }
    let content = r"interface Literal1Example {
    [key: `prefix${string}`]: number | string;
    /** Something else */
    [key: `prefix${number}`]: number;
}
function literal1Example(e: Literal1Example) {
    console.log(e./*literal1*/prefixMember);
    console.log(e./*literal2*/anything);
    console.log(e./*literal3*/prefix0);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "literal1",
        "(index) Literal1Example[`prefix${string}`]: string | number",
        "",
    );
    f.verify_quick_info_at(t, "literal2", "any", "");
    f.verify_quick_info_at(
        t,
        "literal3",
        "(index) Literal1Example[`prefix${string}` | `prefix${number}`]: number",
        "Something else",
    );
    done();
}
