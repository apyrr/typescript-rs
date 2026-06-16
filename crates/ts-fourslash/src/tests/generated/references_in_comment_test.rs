#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_in_comment() {
    let mut t = TestingT;
    run_test_references_in_comment(&mut t);
}

fn run_test_references_in_comment(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesInComment") {
        return;
    }
    let content = r"// References to /*1*/foo or b/*2*/ar
/* in comments should not find fo/*3*/o or bar/*4*/ */
class foo { }
var bar = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
