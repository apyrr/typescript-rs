#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_of_anonymous_function2() {
    let mut t = TestingT;
    run_test_get_occurrences_of_anonymous_function2(&mut t);
}

fn run_test_get_occurrences_of_anonymous_function2(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesOfAnonymousFunction2") {
        return;
    }
    let content = r"//global foo definition
function foo() {}

(function f/*local*/oo(): number {
    return foo(); // local foo reference
})
//global foo references
fo/*global*/o();
var f = foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![
            MarkerOrRangeOrName::Name("local".to_string()),
            MarkerOrRangeOrName::Name("global".to_string()),
        ],
    );
    done();
}
