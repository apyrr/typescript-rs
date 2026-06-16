#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_package_json_imports_caps_in_path1() {
    let mut t = TestingT;
    run_test_auto_import_package_json_imports_caps_in_path1(&mut t);
}

fn run_test_auto_import_package_json_imports_caps_in_path1(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportPackageJsonImports_capsInPath1") {
        return;
    }
    let content = r##"// @module: node18
// @Filename: /Dev/package.json
{
  "imports": {
    "#thing": "./src/something.js"
  }
}
// @Filename: /Dev/src/something.ts
export function something(name: string): any;
// @Filename: /Dev/a.ts
something/**/"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["#thing".to_string()], None);
    done();
}
