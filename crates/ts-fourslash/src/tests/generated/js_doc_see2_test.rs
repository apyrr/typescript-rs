#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_see2() {
    let mut t = TestingT;
    run_test_js_doc_see2(&mut t);
}

fn run_test_js_doc_see2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocSee2") {
        return;
    }
    let content = r#"/** @see {/*use1*/[|foooo|]} unknown reference*/
const a = ""
/** @see {/*use2*/[|@bar|]} invalid tag*/
const b = ""
/** @see /*use3*/[|foooo|] unknown reference without brace*/
const c = ""
/** @see /*use4*/[|@bar|] invalid tag without brace*/
const [|/*def1*/d|] = ""
/** @see {/*use5*/[|d@fff|]} partial reference */
const e = ""
/** @see /*use6*/[|@@@@@@|] total invalid tag*/
const f = ""
/** @see d@{/*use7*/[|fff|]} partial reference */
const g = """#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "use1".to_string(),
            "use2".to_string(),
            "use3".to_string(),
            "use4".to_string(),
            "use5".to_string(),
            "use6".to_string(),
            "use7".to_string(),
        ],
    );
    done();
}
