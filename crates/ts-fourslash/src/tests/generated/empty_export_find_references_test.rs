#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_empty_export_find_references() {
    let mut t = TestingT;
    run_test_empty_export_find_references(&mut t);
}

fn run_test_empty_export_find_references(t: &mut TestingT) {
    if should_skip_if_failing("TestEmptyExportFindReferences") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
/**/module.exports = {

}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(t, None, vec![MarkerOrRangeOrName::Name("".to_string())]);
    done();
}
