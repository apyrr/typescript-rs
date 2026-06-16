#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_non_string_import_assertion() {
    let mut t = TestingT;
    run_test_get_occurrences_non_string_import_assertion(&mut t);
}

fn run_test_get_occurrences_non_string_import_assertion(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: node18
import * as react from "react" with { cache: /**/0 };
react.Children;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(t, None, vec![MarkerOrRangeOrName::Name("".to_string())]);
    done();
}
