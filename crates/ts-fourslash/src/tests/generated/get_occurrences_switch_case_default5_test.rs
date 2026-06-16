#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_switch_case_default5() {
    let mut t = TestingT;
    run_test_get_occurrences_switch_case_default5(&mut t);
}

fn run_test_get_occurrences_switch_case_default5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"switch/*1*/ (10) {
    case/*2*/ 1:
    case/*3*/ 2:
    case/*4*/ 4:
    case/*5*/ 8:
        foo: switch/*6*/ (20) {
            case/*7*/ 1:
            case/*8*/ 2:
                break/*9*/;
            default/*10*/:
                break foo;
        }
    case/*11*/ 0xBEEF:
    default/*12*/:
        break/*13*/;
    case 16/*14*/:
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
