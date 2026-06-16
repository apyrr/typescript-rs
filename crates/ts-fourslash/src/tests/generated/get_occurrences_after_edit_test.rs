#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_after_edit() {
    let mut t = TestingT;
    run_test_get_occurrences_after_edit(&mut t);
}

fn run_test_get_occurrences_after_edit(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesAfterEdit") {
        return;
    }
    let content = r"/*0*/
interface A {
    foo: string;
}
function foo(x: A) {
    x.f/*1*/oo
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Name("1".to_string())],
    );
    f.go_to_marker(t, "0");
    f.insert(t, "\n");
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Name("1".to_string())],
    );
    done();
}
