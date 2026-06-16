#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_add_all_missing_imports() {
    let mut t = TestingT;
    run_test_import_name_code_fix_add_all_missing_imports(&mut t);
}

fn run_test_import_name_code_fix_add_all_missing_imports(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_add_all_missing_imports") {
        return;
    }
    let content = r"// @Filename: /a.ts
export const a: number;
// @Filename: /b.ts
export const b: number;
// @Filename: /c.ts
export const c: number;
// @Filename: /main.ts
a;
b;
c;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/main.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import { a } from "./a";
import { b } from "./b";
import { c } from "./c";

a;
b;
c;"#
            .to_string(),
        },
    );
    done();
}
