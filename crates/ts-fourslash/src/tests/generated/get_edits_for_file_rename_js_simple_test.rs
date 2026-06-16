#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_js_simple() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_js_simple(&mut t);
}

fn run_test_get_edits_for_file_rename_js_simple(t: &mut TestingT) {
    if should_skip_if_failing("TestGetEditsForFileRename_js_simple") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: /a.js
import b from "./b.js";
// @Filename: /b.js
module.exports = 1;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/b.js",
        "/c.js",
        std::collections::HashMap::from([(
            "/a.js".to_string(),
            r#"import b from "./c.js";"#.to_string(),
        )]),
    );
    done();
}
