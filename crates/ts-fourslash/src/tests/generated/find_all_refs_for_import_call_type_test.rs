#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_import_call_type() {
    let mut t = TestingT;
    run_test_find_all_refs_for_import_call_type(&mut t);
}

fn run_test_find_all_refs_for_import_call_type(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForImportCallType") {
        return;
    }
    let content = r#"// @Filename: /app.ts
export function he/**/llo() {};
// @Filename: /re-export.ts
export type app = typeof import("./app")
// @Filename: /indirect-use.ts
import type { app } from "./re-export";
declare const app: app
app.hello();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
