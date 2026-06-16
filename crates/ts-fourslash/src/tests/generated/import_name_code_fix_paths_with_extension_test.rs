#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_paths_with_extension() {
    let mut t = TestingT;
    run_test_import_name_code_fix_paths_with_extension(&mut t);
}

fn run_test_import_name_code_fix_paths_with_extension(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_pathsWithExtension") {
        return;
    }
    let content = r##"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "target": "ESNext",
    "module": "Node16",
    "moduleResolution": "Node16",
    "rootDir": "./src",
    "outDir": "./dist",
    "paths": {
      "#internals/*": ["./src/internals/*.ts"]
    }
  },
  "include": ["src"]
}
// @Filename: /src/internals/example.ts
export function helloWorld() {}
// @Filename: /src/index.ts
helloWorld/**/"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["#internals/example".to_string()],
        Some(UserPreferences {
            import_module_specifier_ending:
                modulespecifiers::ImportModuleSpecifierEndingPreference::Js,
            ..Default::default()
        }),
    );
    done();
}
