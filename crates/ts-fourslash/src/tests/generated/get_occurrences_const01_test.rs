#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_const01() {
    let mut t = TestingT;
    run_test_get_occurrences_const01(&mut t);
}

fn run_test_get_occurrences_const01(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesConst01") {
        return;
    }
    let content = r"[|const|] enum E1 {
    v1,
    v2
}

/*2*/const c = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Name("2".to_string())],
    );
    done();
}
