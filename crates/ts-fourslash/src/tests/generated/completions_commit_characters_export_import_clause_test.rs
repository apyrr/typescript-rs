#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_commit_characters_export_import_clause() {
    let mut t = TestingT;
    run_test_completions_commit_characters_export_import_clause(&mut t);
}

fn run_test_completions_commit_characters_export_import_clause(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsCommitCharactersExportImportClause") {
        return;
    }
    let content = r#"// @filename: a.ts
const xx: string = "aa";
function ff(): void {}
export { /*1*/ };
// @filename: exports.ts
export const ff: string = "";
export const aa = () => {};
// @filename: imports.ts
import { /*2*/ } from "./exports";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
