#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_constructor() {
    let mut t = TestingT;
    run_test_get_occurrences_constructor(&mut t);
}

fn run_test_get_occurrences_constructor(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    [|const/**/ructor|]();
    [|constructor|](x: number);
    [|constructor|](y: string, x: number);
    [|constructor|](a?: any, ...r: any[]) {
        if (a === undefined && r.length === 0) {
            return;
        }

        return;
    }
}

class D {
    constructor(public x: number, public y: number) {
    }
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
