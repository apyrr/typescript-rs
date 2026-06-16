#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlight_in_export1() {
    let mut t = TestingT;
    run_test_document_highlight_in_export1(&mut t);
}

fn run_test_document_highlight_in_export1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class [|C|] {}
[|export|] { [|C|] [|as|] [|D|] };";
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
