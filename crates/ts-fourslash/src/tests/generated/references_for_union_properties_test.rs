#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_union_properties() {
    let mut t = TestingT;
    run_test_references_for_union_properties(&mut t);
}

fn run_test_references_for_union_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForUnionProperties") {
        return;
    }
    let content = r"interface One {
    common: { /*one*/a: number; };
}

interface Base {
    /*base*/a: string;
    b: string;
}

interface HasAOrB extends Base {
    a: string;
    b: string;
}

interface Two {
    common: HasAOrB;
}

var x : One | Two;

x.common./*x*/a;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &["one".to_string(), "base".to_string(), "x".to_string()],
    );
    done();
}
