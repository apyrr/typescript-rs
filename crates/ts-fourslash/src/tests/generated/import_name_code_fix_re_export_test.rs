#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_re_export() {
    let mut t = TestingT;
    run_test_import_name_code_fix_re_export(&mut t);
}

fn run_test_import_name_code_fix_re_export(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixReExport") {
        return;
    }
    let content = r#"// @Filename: /a.ts
export const x = 0";
// @Filename: /b.ts
[|export { x } from "./a";
x;|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_range_after_code_fix(
        t,
        "import { x } from \"./a\";\n\nexport { x } from \"./a\";\nx;",
        true,
        0,
        0,
    );
    done();
}
