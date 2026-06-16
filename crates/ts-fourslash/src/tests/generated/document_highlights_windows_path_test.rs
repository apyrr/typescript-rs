#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights_windows_path() {
    let mut t = TestingT;
    run_test_document_highlights_windows_path(&mut t);
}

fn run_test_document_highlights_windows_path(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlights_windowsPath") {
        return;
    }
    let content = r"//@Filename: C:\a\b\c.ts
var /*1*/[|x|] = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights_with_options(
        t,
        None,
        vec![f.ranges()[0].file_name().to_string()],
        vec![MarkerOrRangeOrName::Range(f.ranges()[0].clone())],
    );
    done();
}
