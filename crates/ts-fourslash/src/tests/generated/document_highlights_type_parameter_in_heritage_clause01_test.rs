#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights_type_parameter_in_heritage_clause01() {
    let mut t = TestingT;
    run_test_document_highlights_type_parameter_in_heritage_clause01(&mut t);
}

fn run_test_document_highlights_type_parameter_in_heritage_clause01(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlightsTypeParameterInHeritageClause01") {
        return;
    }
    let content = r"// @lib: es5
interface I<[|T|]> extends I<[|T|]>, [|T|] {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
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
