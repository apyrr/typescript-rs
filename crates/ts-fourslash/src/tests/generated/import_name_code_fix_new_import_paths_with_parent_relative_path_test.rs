#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_paths_with_parent_relative_path() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_paths_with_parent_relative_path(&mut t);
}

fn run_test_import_name_code_fix_new_import_paths_with_parent_relative_path(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /src/a.ts
[|foo|]
// @Filename: /thisHasPathMapping.ts
export function foo() {};
// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "baseUrl": "src",
        "paths": {
            "foo": ["..\\thisHasPathMapping"]
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { foo } from "foo";

foo"#
                .to_string(),
        ],
        None,
    );
    done();
}
