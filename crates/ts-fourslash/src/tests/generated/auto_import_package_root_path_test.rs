#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_package_root_path() {
    let mut t = TestingT;
    run_test_auto_import_package_root_path(&mut t);
}

fn run_test_auto_import_package_root_path(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportPackageRootPath") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: /node_modules/pkg/package.json
{
    "name": "pkg",
    "version": "1.0.0",
    "main": "lib",
    "module": "lib"
 }
// @Filename: /node_modules/pkg/lib/index.js
export function foo() {};
// @Filename: /package.json
{
    "dependencies": {
       "pkg": "*"
    }
 }
// @Filename: /index.ts
foo/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["pkg".to_string()], None);
    done();
}
