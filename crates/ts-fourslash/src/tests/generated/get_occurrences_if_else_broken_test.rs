#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_if_else_broken() {
    let mut t = TestingT;
    run_test_get_occurrences_if_else_broken(&mut t);
}

fn run_test_get_occurrences_if_else_broken(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"[|if|] (true) {
    var x = 1;
}
[|else     if|] ()
[|else if|]
[|else|]  /*  whar garbl   */   [|if|] (i/**/f (true) { } else { })
else";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    f.verify_baseline_document_highlights(t, None, vec![MarkerOrRangeOrName::Name("".to_string())]);
    done();
}
