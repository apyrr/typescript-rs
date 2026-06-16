#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlight_at_inherited_properties6() {
    let mut t = TestingT;
    run_test_document_highlight_at_inherited_properties6(&mut t);
}

fn run_test_document_highlight_at_inherited_properties6(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: file1.ts
class C extends D {
    [|prop0|]: string;
    [|prop1|]: string;
}

class D extends C {
    [|prop0|]: string;
    [|prop1|]: string;
}

var d: D;
d.[|prop1|];";
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
