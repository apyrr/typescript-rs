#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights_invalid_modifier_locations() {
    let mut t = TestingT;
    run_test_document_highlights_invalid_modifier_locations(&mut t);
}

fn run_test_document_highlights_invalid_modifier_locations(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlightsInvalidModifierLocations") {
        return;
    }
    let content = r"class C {
    m([|readonly|] p) {}
}
function f([|readonly|] p) {}

class D {
    m([|public|] p) {}
}
function g([|public|] p) {}";
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
