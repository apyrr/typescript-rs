#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_switch_case_default2() {
    let mut t = TestingT;
    run_test_get_occurrences_switch_case_default2(&mut t);
}

fn run_test_get_occurrences_switch_case_default2(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesSwitchCaseDefault2") {
        return;
    }
    let content = r"switch (10) {
    case 1:
    case 2:
    case 4:
    case 8:
        foo: [|switch|] (20) {
            [|case|] 1:
            [|case|] 2:
                [|break|];
            [|default|]:
                [|break|] foo;
        }
    case 0xBEEF:
    default:
        break;
    case 16:
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
