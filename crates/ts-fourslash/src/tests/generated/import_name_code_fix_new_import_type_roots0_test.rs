#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_type_roots0() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_type_roots0(&mut t);
}

fn run_test_import_name_code_fix_new_import_type_roots0(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportTypeRoots0") {
        return;
    }
    let content = r#"// @Filename: a/f1.ts
[|foo/*0*/();|]
// @Filename: types/random/index.ts
export function foo() {};
// @Filename: tsconfig.json
{
    "compilerOptions": {
        "typeRoots": [
            "./types"
        ]
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { foo } from "../types/random";

foo();"#
                .to_string(),
        ],
        None,
    );
    done();
}
