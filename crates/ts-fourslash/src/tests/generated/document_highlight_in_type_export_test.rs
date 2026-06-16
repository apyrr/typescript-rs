#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlight_in_type_export() {
    let mut t = TestingT;
    run_test_document_highlight_in_type_export(&mut t);
}

fn run_test_document_highlight_in_type_export(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlightInTypeExport") {
        return;
    }
    let content = r"// @Filename: /1.ts
type [|A|] = 1;
export { [|A|] as [|B|] };
// @Filename: /2.ts
type [|A|] = 1;
let [|A|]: [|A|] = 1;
export { [|A|] as [|B|] };
// @Filename: /3.ts
type [|A|] = 1;
let [|A|]: [|A|] = 1;
export type { [|A|] as [|B|] };";
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
