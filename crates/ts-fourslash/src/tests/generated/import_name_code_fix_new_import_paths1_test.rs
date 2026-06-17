#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_paths1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_paths1(&mut t);
}

fn run_test_import_name_code_fix_new_import_paths1(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportPaths1") {
        return;
    }
    let content = r#"[|foo/*0*/();|]
// @Filename: folder_b/f2.ts
export function foo() {};
// @Filename: tsconfig.json
{
    "compilerOptions": {
        "baseUrl": ".",
        "paths": {
            "b/*": [ "folder_b/*" ]
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { foo } from "b/f2";

foo();"#
            .to_string()],
        None,
    );
    done();
}
