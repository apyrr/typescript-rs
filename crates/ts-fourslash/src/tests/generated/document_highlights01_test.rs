#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights01() {
    let mut t = TestingT;
    run_test_document_highlights01(&mut t);
}

fn run_test_document_highlights01(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlights01") {
        return;
    }
    let content = r"// @lib: es5
// @Filename: a.ts
function [|f|](x: typeof [|f|]) {
    [|f|]([|f|]);
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
