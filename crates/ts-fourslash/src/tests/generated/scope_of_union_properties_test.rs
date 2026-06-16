#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_scope_of_union_properties() {
    let mut t = TestingT;
    run_test_scope_of_union_properties(&mut t);
}

fn run_test_scope_of_union_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestScopeOfUnionProperties") {
        return;
    }
    let content = r"function f(s: string | number) {
    s.constr/*1*/uctor
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Name("1".to_string())],
    );
    done();
}
