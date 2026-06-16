#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_try_catch_finally3() {
    let mut t = TestingT;
    run_test_get_occurrences_try_catch_finally3(&mut t);
}

fn run_test_get_occurrences_try_catch_finally3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"try {
    try {
    }
    catch (x) {
    }

    [|t/*1*/r/*2*/y|] {
    }
    [|finall/*3*/y|] {
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
