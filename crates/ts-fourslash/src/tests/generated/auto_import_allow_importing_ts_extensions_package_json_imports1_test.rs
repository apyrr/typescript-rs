#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_allow_importing_ts_extensions_package_json_imports1() {
    let mut t = TestingT;
    run_test_auto_import_allow_importing_ts_extensions_package_json_imports1(&mut t);
}

fn run_test_auto_import_allow_importing_ts_extensions_package_json_imports1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r##"// @lib: es5
// @module: node18
// @allowImportingTsExtensions: true
// @Filename: /node_modules/pkg/package.json
{
  "name": "pkg",
  "type": "module",
  "exports": {
    "./*": {
      "types": "./types/*",
      "default": "./dist/*"
    }
  }
}
// @Filename: /node_modules/pkg/types/external.d.ts
export declare function external(name: string): any;
// @Filename: /package.json
{
  "name": "self",
  "type": "module",
  "imports": {
    "#*": "./src/*"
  },
  "dependencies": {
    "pkg": "*"
  }
}
// @Filename: /src/add.ts
export function add(a: number, b: number) {}
// @Filename: /src/index.ts
add/*imports*/;
external/*exports*/;"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "imports", &vec!["#add.ts".to_string()], None);
    f.verify_import_fix_module_specifiers(t, "exports", &vec!["pkg/external.js".to_string()], None);
    done();
}
