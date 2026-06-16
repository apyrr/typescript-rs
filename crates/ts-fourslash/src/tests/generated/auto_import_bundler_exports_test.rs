#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_bundler_exports() {
    let mut t = TestingT;
    run_test_auto_import_bundler_exports(&mut t);
}

fn run_test_auto_import_bundler_exports(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportBundlerExports") {
        return;
    }
    let content = r#"// @module: esnext
// @moduleResolution: bundler
// @Filename: /node_modules/dep/package.json
{
  "name": "dep",
  "version": "1.0.0",
  "exports": {
    ".": "./dist/index.js"
  }
}
// @Filename: /node_modules/dep/dist/index.d.ts
export const dep: number;
// @Filename: /index.ts
dep/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["dep".to_string()], None);
    done();
}
