#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_if_else5() {
    let mut t = TestingT;
    run_test_get_occurrences_if_else5(&mut t);
}

fn run_test_get_occurrences_if_else5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"if/*1*/ (true) {
    if/*2*/ (false) {
    }
    else/*3*/ {
    }
    if/*4*/ (true) {
    }
    else/*5*/ {
        if/*6*/ (false)
            if/*7*/ (true)
                var x = undefined;
    }
}
else/*8*/            if (null) {
}
else/*9*/ /* whar garbl */ if/*10*/ (undefined) {
}
else/*11*/
if/*12*/ (false) {
}
else/*13*/ { }";
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
