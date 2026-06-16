#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights02() {
    let mut t = TestingT;
    run_test_document_highlights02(&mut t);
}

fn run_test_document_highlights02(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlights02") {
        return;
    }
    let content = r#"// @lib: es5
// @Filename: a.ts
function [|foo|] () {
	return 1;
}
[|foo|]();
// @Filename: b.ts
/// <reference path="a.ts"/>
[|foo|]();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "a.ts");
    f.go_to_file(t, "b.ts");
    f.verify_baseline_document_highlights_with_options(
        t,
        None,
        vec!["a.ts".to_string(), "b.ts".to_string()],
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    done();
}
