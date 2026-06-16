#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_package_json_imports_conditions() {
    let mut t = TestingT;
    run_test_auto_import_package_json_imports_conditions(&mut t);
}

fn run_test_auto_import_package_json_imports_conditions(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportPackageJsonImportsConditions") {
        return;
    }
    let content = r##"// @module: node18
// @Filename: /package.json
{
  "imports": {
    "#thing": {
        "types": { "import": "./types-esm/thing.d.mts", "require": "./types/thing.d.ts" },
        "default": { "import": "./esm/thing.mjs", "require": "./dist/thing.js" }
     }
  }
}
// @Filename: /src/.ts
something/*a*/
// @Filename: /types/thing.d.ts
export function something(name: string): any;"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "a", &vec!["#thing".to_string()], None);
    done();
}
