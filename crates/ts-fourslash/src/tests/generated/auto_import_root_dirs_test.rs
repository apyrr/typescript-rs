#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_root_dirs() {
    let mut t = TestingT;
    run_test_auto_import_root_dirs(&mut t);
}

fn run_test_auto_import_root_dirs(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportRootDirs") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "module": "commonjs",
        "rootDirs": [".", "./some/other/root"]
    }
}
// @Filename: /some/other/root/types.ts
export type Something = {};
// @Filename: /index.ts
const s: Something/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["./types".to_string()], None);
    done();
}
