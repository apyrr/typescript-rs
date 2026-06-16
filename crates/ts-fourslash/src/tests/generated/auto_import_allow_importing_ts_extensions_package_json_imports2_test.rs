#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_allow_importing_ts_extensions_package_json_imports2() {
    let mut t = TestingT;
    run_test_auto_import_allow_importing_ts_extensions_package_json_imports2(&mut t);
}

fn run_test_auto_import_allow_importing_ts_extensions_package_json_imports2(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportAllowImportingTsExtensionsPackageJsonImports2") {
        return;
    }
    let content = r##"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "module": "nodenext",
    "allowImportingTsExtensions": true,
    "rootDir": "src",
    "outDir": "dist",
    "declarationDir": "types",
    "declaration": true
  }
}
// @Filename: /package.json
{
  "name": "self",
  "type": "module",
  "imports": {
    "#*": {
      "types": "./types/*",
      "default": "./dist/*"
    }
  }
}
// @Filename: /src/add.ts
export function add(a: number, b: number) {}
// @Filename: /src/index.ts
add/*imports*/;
external/*exports*/;"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "imports", &vec!["#add.js".to_string()], None);
    done();
}
