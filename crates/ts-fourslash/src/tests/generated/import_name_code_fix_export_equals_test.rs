#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_export_equals() {
    let mut t = TestingT;
    run_test_import_name_code_fix_export_equals(&mut t);
}

fn run_test_import_name_code_fix_export_equals(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_exportEquals") {
        return;
    }
    let content = r"// @module: commonjs
// @esModuleInterop: false
// @allowSyntheticDefaultImports: false
// @Filename: /a.d.ts
declare function a(): void;
declare namespace a {
    export interface b {}
}
export = a;
// @Filename: /b.ts
a;
let x: b;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import { b } from "./a";
import a = require("./a");

a;
let x: b;"#
                .to_string(),
        },
    );
    done();
}
