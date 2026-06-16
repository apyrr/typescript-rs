#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_type_only2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_type_only2(&mut t);
}

fn run_test_import_name_code_fix_type_only2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_typeOnly2") {
        return;
    }
    let content = r"// @importsNotUsedAsValues: error
// @Filename: types.ts
export class A {}
// @Filename: index.ts
const a: A = new A();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "index.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import { A } from "./types";

const a: A = new A();"#
                .to_string(),
        },
    );
    done();
}
