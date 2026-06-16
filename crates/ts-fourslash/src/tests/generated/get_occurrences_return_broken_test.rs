#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_return_broken() {
    let mut t = TestingT;
    run_test_get_occurrences_return_broken(&mut t);
}

fn run_test_get_occurrences_return_broken(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"ret/*1*/urn;
retu/*2*/rn;
function f(a: number) {
    if (a > 0) {
        return (function () {
            () => [|return|];
            [|return|];
            [|return|];

            if (false) {
                [|return|] true;
            }
        })() || true;
    }

    var unusued = [1, 2, 3, 4].map(x => { return 4 })

    return;
    return true;
}

class A {
    ret/*3*/urn;
    r/*4*/eturn 8675309;
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
