#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_infer_ending_preference_classic() {
    let mut t = TestingT;
    run_test_import_name_code_fix_infer_ending_preference_classic(&mut t);
}

fn run_test_import_name_code_fix_infer_ending_preference_classic(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixInferEndingPreference_classic") {
        return;
    }
    let content = r#"// @module: esnext
// @checkJs: true
// @allowJs: true
// @noEmit: true
// @Filename: /a.js
export const a = 0;
// @Filename: /b.js
export const b = 0;
// @Filename: /c.js
import { a } from "./a.js";

b/**/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["./b.js".to_string()], None);
    done();
}
