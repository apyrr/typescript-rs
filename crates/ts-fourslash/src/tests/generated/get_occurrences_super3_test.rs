#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_super3() {
    let mut t = TestingT;
    run_test_get_occurrences_super3(&mut t);
}

fn run_test_get_occurrences_super3(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesSuper3") {
        return;
    }
    let content = r"let x = {
    a() {
        return [|s/**/uper|].b();
    },
    b() {
        return [|super|].a();
    },
    c: function () {
        return [|super|].a();
    }
    d: () => [|super|].b();
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
    done();
}
