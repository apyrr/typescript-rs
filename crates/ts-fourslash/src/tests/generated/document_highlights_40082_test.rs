#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights_40082() {
    let mut t = TestingT;
    run_test_document_highlights_40082(&mut t);
}

fn run_test_document_highlights_40082(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlights_40082") {
        return;
    }
    let content = r"// @checkJs: true
export = (state, messages) => {
   export [|default|] {
   }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Range(f.ranges()[0].clone())],
    );
    done();
}
