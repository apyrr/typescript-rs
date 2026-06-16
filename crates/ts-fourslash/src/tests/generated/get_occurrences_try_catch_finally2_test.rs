#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_try_catch_finally2() {
    let mut t = TestingT;
    run_test_get_occurrences_try_catch_finally2(&mut t);
}

fn run_test_get_occurrences_try_catch_finally2(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesTryCatchFinally2") {
        return;
    }
    let content = r"try {
    [|t/*1*/r/*2*/y|] {
    }
    [|c/*3*/atch|] (x) {
    }

    try {
    }
    finally {
    }
}
catch (e) {
}
finally {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.markers()
            .into_iter()
            .map(MarkerOrRangeOrName::Marker)
            .collect(),
    );
    done();
}
