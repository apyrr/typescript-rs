#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_not_affected_by_js_file() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_not_affected_by_js_file(&mut t);
}

fn run_test_get_edits_for_file_rename_not_affected_by_js_file(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export const x = 0;
// @Filename: /a.js
exports.x = 0;
// @Filename: /b.ts
import { x } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/a.ts",
        "/a2.ts",
        std::collections::HashMap::from([(
            "/b.ts".to_string(),
            r#"import { x } from "./a2";"#.to_string(),
        )]),
    );
    done();
}
