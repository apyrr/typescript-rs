#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_from_path_mapping() {
    let mut t = TestingT;
    run_test_import_name_code_fix_from_path_mapping(&mut t);
}

fn run_test_import_name_code_fix_from_path_mapping(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_fromPathMapping") {
        return;
    }
    let content = r#"// @Filename: /a.ts
export const foo = 0;
// @Filename: /x/y.ts
foo;
// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "baseUrl": ".",
        "paths": {
            "@root/*": ["*"],
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/x/y.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { foo } from "@root/a";

foo;"#
                .to_string(),
        ],
        None,
    );
    done();
}
