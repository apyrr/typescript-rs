#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_throw7() {
    let mut t = TestingT;
    run_test_get_occurrences_throw7(&mut t);
}

fn run_test_get_occurrences_throw7(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"try {
    [|throw|] 10;

    try {
        throw 10;
    }
    catch (x) {
        [|throw|] 10;
    }
    finally {
        [|throw|] 10;
    }
}
finally {
    [|throw|] 10;
}

[|throw|] 10;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    done();
}
