#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_for_type_alias() {
    let mut t = TestingT;
    run_test_js_doc_for_type_alias(&mut t);
}

fn run_test_js_doc_for_type_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocForTypeAlias") {
        return;
    }
    let content = r"/** DOC */
type /**/T = number";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "type T = number", "DOC");
    done();
}
