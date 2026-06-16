#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_provider_imports_map1() {
    let mut t = TestingT;
    run_test_auto_import_provider_imports_map1(&mut t);
}

fn run_test_auto_import_provider_imports_map1(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportProvider_importsMap1") {
        return;
    }
    let content = r##"// @Filename: /home/src/workspaces/project/tsconfig.json
{
  "compilerOptions": {
    "module": "nodenext",
    "lib": ["es5"],
    "rootDir": "src",
    "outDir": "dist"
  }
}
// @Filename: /home/src/workspaces/project/package.json
{
  "type": "module",
  "imports": {
    "#is-browser": {
      "browser": "./dist/env/browser.js",
      "default": "./dist/env/node.js"
    }
  }
}
// @Filename: /home/src/workspaces/project/src/env/browser.ts
export const isBrowser = true;
// @Filename: /home/src/workspaces/project/src/env/node.ts
export const isBrowser = false;
// @Filename: /home/src/workspaces/project/src/a.ts
isBrowser/**/"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["#is-browser".to_string(), "./env/browser.js".to_string()],
        None,
    );
    done();
}
