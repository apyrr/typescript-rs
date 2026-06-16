#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_see4() {
    let mut t = TestingT;
    run_test_js_doc_see4(&mut t);
}

fn run_test_js_doc_see4(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocSee4") {
        return;
    }
    let content = r"class [|/*def1*/A|] {
    foo () { }
}
declare const [|/*def2*/a|]: A;
/**
 * @see {/*use1*/[|A|]#foo}
 */
const t1 = 1
/**
 * @see {/*use2*/[|a|].foo()}
 */
const t2 = 1
/**
 * @see {@link /*use3*/[|a|].foo()}
 */
const t3 = 1";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &["use1".to_string(), "use2".to_string(), "use3".to_string()],
    );
    done();
}
