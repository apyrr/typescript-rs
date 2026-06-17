#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_paths_with_leading_dot_slash() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_paths_with_leading_dot_slash(&mut t);
}

fn run_test_import_name_code_fix_new_import_paths_with_leading_dot_slash(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportPaths_withLeadingDotSlash") {
        return;
    }
    let content = r#"// @Filename: /a.ts
[|foo|]
// @Filename: /thisHasPathMapping.ts
export function foo() {};
// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "baseUrl": ".",
        "paths": {
            "foo": ["././thisHasPathMapping"]
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { foo } from "foo";

foo"#
            .to_string()],
        None,
    );
    done();
}
