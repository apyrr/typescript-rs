#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlights_module_import_files_to_search() {
    let mut t = TestingT;
    run_test_document_highlights_module_import_files_to_search(&mut t);
}

fn run_test_document_highlights_module_import_files_to_search(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/@types/foo/index.d.ts
export const x: number;
// @Filename: /a.ts
import * as foo from "foo";
foo.[|x|];
// @Filename: /b.ts
import { [|x|] } from "foo";
// @Filename: /c.ts
import { x } from "foo";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights_with_options(
        t,
        None,
        vec!["/a.ts".to_string(), "/b.ts".to_string()],
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    done();
}
