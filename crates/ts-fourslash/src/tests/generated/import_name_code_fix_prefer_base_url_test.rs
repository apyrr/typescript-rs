#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_prefer_base_url() {
    let mut t = TestingT;
    run_test_import_name_code_fix_prefer_base_url(&mut t);
}

fn run_test_import_name_code_fix_prefer_base_url(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_preferBaseUrl") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
{ "compilerOptions": { "baseUrl": "./src" } }
// @Filename: /src/d0/d1/d2/file.ts
foo/**/;
// @Filename: /src/d0/a.ts
export const foo = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/src/d0/d1/d2/file.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { foo } from "d0/a";

foo;"#
            .to_string()],
        None,
    );
    done();
}
