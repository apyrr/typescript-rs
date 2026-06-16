#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights_files_to_search() {
    let mut t = TestingT;
    run_test_document_highlights_files_to_search(&mut t);
}

fn run_test_document_highlights_files_to_search(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlights_filesToSearch") {
        return;
    }
    let content = r#"// @Filename: /a.ts
export const [|x|] = 0;
// @Filename: /b.ts
import { [|x|] } from "./a";"#;
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
