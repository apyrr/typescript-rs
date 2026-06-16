#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_set_and_get3() {
    let mut t = TestingT;
    run_test_get_occurrences_set_and_get3(&mut t);
}

fn run_test_get_occurrences_set_and_get3(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesSetAndGet3") {
        return;
    }
    let content = r"class Foo {
    set bar(b: any) {
    }

    public get bar(): any {
        return undefined;
    }

    public set set(s: any) {
    }

    public get set(): any {
        return undefined;
    }

    public [|set|] get(g: any) {
    }

    public [|get|] get(): any {
        return undefined;
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
