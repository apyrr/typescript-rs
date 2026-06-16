#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_const03() {
    let mut t = TestingT;
    run_test_get_occurrences_const03(&mut t);
}

fn run_test_get_occurrences_const03(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesConst03") {
        return;
    }
    let content = r"namespace m {
    export /*1*/const x;
    export [|const|] enum E {
    }
}

export /*2*/const x;
export [|const|] enum E {
}";
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
        f.markers()
            .into_iter()
            .map(MarkerOrRangeOrName::Marker)
            .collect(),
    );
    done();
}
