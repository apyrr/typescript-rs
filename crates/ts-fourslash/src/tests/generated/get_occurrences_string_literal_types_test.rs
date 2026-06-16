#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_string_literal_types() {
    let mut t = TestingT;
    run_test_get_occurrences_string_literal_types(&mut t);
}

fn run_test_get_occurrences_string_literal_types(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function foo(a: "[|option 1|]") { }
foo("[|option 1|]");"#;
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
