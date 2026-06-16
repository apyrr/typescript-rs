#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_of_undefined_symbol() {
    let mut t = TestingT;
    run_test_get_occurrences_of_undefined_symbol(&mut t);
}

fn run_test_get_occurrences_of_undefined_symbol(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesOfUndefinedSymbol") {
        return;
    }
    let content = r"var obj1: {
    (bar: any): any;
    new (bar: any): any;
    [bar: any]: any;
    bar: any;
    foob(bar: any): any;
};

class cls3 {
    property zeFunc() {
    super.ceFun/**/c();
}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(t, None, vec![MarkerOrRangeOrName::Name("".to_string())]);
    done();
}
