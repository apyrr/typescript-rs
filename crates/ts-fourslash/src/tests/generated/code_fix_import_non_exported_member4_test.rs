#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_import_non_exported_member4() {
    let mut t = TestingT;
    run_test_code_fix_import_non_exported_member4(&mut t);
}

fn run_test_code_fix_import_non_exported_member4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: esnext
// @filename: /a.d.ts
declare function foo(): any;
declare function bar(): any;
// @filename: /b.ts
import { bar } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_code_fix_not_available(t, &vec!["fixImportNonExportedMember".to_string()]);
    done();
}
