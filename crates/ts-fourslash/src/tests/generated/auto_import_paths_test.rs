#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_paths() {
    let mut t = TestingT;
    run_test_auto_import_paths(&mut t);
}

fn run_test_auto_import_paths(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportPaths") {
        return;
    }
    let content = r#"// @Filename: /package1/jsconfig.json
{
  "compilerOptions": {
    checkJs: true,
    "paths": {
      "package1/*": ["./*"],
      "package2/*": ["../package2/*"]
    },
    "baseUrl": "."
  },
  "include": [
    ".",
    "../package2"
  ]
}
// @Filename: /package1/file1.js
bar/**/
// @Filename: /package2/file1.js
export const bar = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["package2/file1".to_string()],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::Shortest,
            ..Default::default()
        }),
    );
    done();
}
