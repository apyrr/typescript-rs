#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_switch_case_default_broken() {
    let mut t = TestingT;
    run_test_get_occurrences_switch_case_default_broken(&mut t);
}

fn run_test_get_occurrences_switch_case_default_broken(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesSwitchCaseDefaultBroken") {
        return;
    }
    let content = r"swi/*1*/tch(10) {
    case 1:
    case 2:
    c/*2*/ase 4:
    case 8:
    case 0xBEEF:
    de/*4*/fult:
        break;
    /*5*/cas 16:
    c/*3*/ase 12:
        function f() {
            br/*11*/eak;
            /*12*/break;
        }
}

sw/*6*/itch (10) {
    de/*7*/fault
    case 1:
    case 2

    c/*8*/ose 4:
    case 8:
    case 0xBEEF:
        bre/*9*/ak;
    case 16:
        () => bre/*10*/ak;
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
