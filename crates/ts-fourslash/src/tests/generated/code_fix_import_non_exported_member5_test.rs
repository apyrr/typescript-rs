#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_import_non_exported_member5() {
    let mut t = TestingT;
    run_test_code_fix_import_non_exported_member5(&mut t);
}

fn run_test_code_fix_import_non_exported_member5(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixImportNonExportedMember5") {
        return;
    }
    let content = r#"// @moduleResolution: bundler
// @module: esnext
// @filename: /node_modules/foo/index.js
function bar() {}
// @filename: /b.ts
import { bar } from "./foo";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_code_fix_not_available(t, &vec!["fixImportNonExportedMember".to_string()]);
    done();
}
