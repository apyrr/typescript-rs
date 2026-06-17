#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_paths0() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_paths0(&mut t);
}

fn run_test_import_name_code_fix_new_import_paths0(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportPaths0") {
        return;
    }
    let content = r#"[|foo/*0*/();|]
// @Filename: folder_a/f2.ts
export function foo() {};
// @Filename: tsconfig.json
{
    "compilerOptions": {
        "baseUrl": ".",
        "paths": {
            "a": [ "folder_a/f2" ]
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { foo } from "a";

foo();"#
            .to_string()],
        None,
    );
    done();
}
