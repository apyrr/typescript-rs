#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_property_in_aliased_interface() {
    let mut t = TestingT;
    run_test_get_occurrences_property_in_aliased_interface(&mut t);
}

fn run_test_get_occurrences_property_in_aliased_interface(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesPropertyInAliasedInterface") {
        return;
    }
    let content = r"namespace m {
    export interface Foo {
        [|abc|]
    }
}

import Bar = m.Foo;

export interface I extends Bar {
    [|abc|]
}

class C implements Bar {
    [|abc|]
}

(new C()).[|abc|];";
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
