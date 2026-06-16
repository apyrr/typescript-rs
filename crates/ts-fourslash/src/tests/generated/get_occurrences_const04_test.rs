#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_const04() {
    let mut t = TestingT;
    run_test_get_occurrences_const04(&mut t);
}

fn run_test_get_occurrences_const04(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesConst04") {
        return;
    }
    let content = r"export const class C {
    private static c/*1*/onst f/*2*/oo;
    constructor(public con/*3*/st foo) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![
            MarkerOrRangeOrName::Name("1".to_string()),
            MarkerOrRangeOrName::Name("2".to_string()),
            MarkerOrRangeOrName::Name("3".to_string()),
        ],
    );
    done();
}
