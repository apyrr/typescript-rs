#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_barrel_export() {
    let mut t = TestingT;
    run_test_import_name_code_fix_barrel_export(&mut t);
}

fn run_test_import_name_code_fix_barrel_export(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_barrelExport") {
        return;
    }
    let content = r#"// @module: commonjs
// @Filename: /foo/a.ts
export const A = 0;
// @Filename: /foo/b.ts
export {};
A/*sibling*/
// @Filename: /foo/index.ts
export * from "./a";
export * from "./b";
// @Filename: /index.ts
export * from "./foo";
export * from "./src";
// @Filename: /src/a.ts
export {};
A/*parent*/
// @Filename: /src/index.ts
export * from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "sibling",
        &vec!["./a".to_string(), ".".to_string(), "..".to_string()],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "parent",
        &vec![
            "../foo".to_string(),
            "../foo/a".to_string(),
            "..".to_string(),
        ],
        None,
    );
    done();
}
