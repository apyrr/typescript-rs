#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_see_rename1() {
    let mut t = TestingT;
    run_test_js_doc_see_rename1(&mut t);
}

fn run_test_js_doc_see_rename1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocSee_rename1") {
        return;
    }
    let content = r#"[|interface [|{| "contextRangeIndex": 0 |}A|] {}|]
/**
 * @see {[|A|]}
 */
declare const a: [|A|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        f.ranges()[1..].iter().cloned().map(Into::into).collect(),
    );
    done();
}
