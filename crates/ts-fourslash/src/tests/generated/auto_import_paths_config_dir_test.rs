#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_paths_config_dir() {
    let mut t = TestingT;
    run_test_auto_import_paths_config_dir(&mut t);
}

fn run_test_auto_import_paths_config_dir(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportPathsConfigDir") {
        return;
    }
    let content = r#"// @Filename: tsconfig.json
{
    "compilerOptions": {
        "paths": {
            "@root/*": ["${configDir}/src/*"]
        }
    }
}
// @Filename: src/one.ts
export const one = 1;
// @Filename: src/foo/two.ts
one/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["@root/one".to_string()], None);
    done();
}
