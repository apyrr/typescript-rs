#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_barrel_export5() {
    let mut t = TestingT;
    run_test_import_name_code_fix_barrel_export5(&mut t);
}

fn run_test_import_name_code_fix_barrel_export5(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_barrelExport5") {
        return;
    }
    let content = r#"// @module: node18
// @Filename: /package.json
{ "type": "module" }
// @Filename: /foo/a.ts
export const A = 0;
// @Filename: /foo/b.ts
export {};
A/*sibling*/
// @Filename: /foo/index.ts
export * from "./a.js";
export * from "./b.js";
// @Filename: /index.ts
export * from "./foo/index.js";
export * from "./src/index.js";
// @Filename: /src/a.ts
export {};
A/*parent*/
// @Filename: /src/index.ts
export * from "./a.js";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "sibling",
        &vec![
            "./a.js".to_string(),
            "./index.js".to_string(),
            "../index.js".to_string(),
        ],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "parent",
        &vec![
            "../foo/a.js".to_string(),
            "../foo/index.js".to_string(),
            "../index.js".to_string(),
        ],
        None,
    );
    done();
}
