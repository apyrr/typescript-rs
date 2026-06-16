#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_inside_templates1() {
    let mut t = TestingT;
    run_test_find_all_refs_inside_templates1(&mut t);
}

fn run_test_find_all_refs_inside_templates1(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsInsideTemplates1") {
        return;
    }
    let content = r"/*1*/var /*2*/x = 10;
var y = `${ /*3*/x } ${ /*4*/x }`";
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
