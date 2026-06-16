#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_see3() {
    let mut t = TestingT;
    run_test_js_doc_see3(&mut t);
}

fn run_test_js_doc_see3(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocSee3") {
        return;
    }
    let content = r"function foo ([|/*def1*/a|]: string) {
    /**
     * @see {/*use1*/[|a|]}
     */
    function bar ([|/*def2*/a|]: string) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["use1".to_string()]);
    done();
}
