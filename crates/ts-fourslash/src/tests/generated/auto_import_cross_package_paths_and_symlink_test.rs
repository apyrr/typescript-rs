#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_cross_package_paths_and_symlink() {
    let mut t = TestingT;
    run_test_auto_import_cross_package_paths_and_symlink(&mut t);
}

fn run_test_auto_import_cross_package_paths_and_symlink(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportCrossPackage_pathsAndSymlink") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/packages/common/package.json
{
  "name": "@company/common",
  "version": "1.0.0",
  "main": "./lib/index.tsx"
}
// @Filename: /home/src/workspaces/project/packages/common/lib/index.tsx
export function Tooltip {};
// @Filename: /home/src/workspaces/project/packages/app/package.json
{
  "name": "@company/app",
  "version": "1.0.0",
  "dependencies": {
    "@company/common": "1.0.0"
  }
}
// @Filename: /home/src/workspaces/project/packages/app/tsconfig.json
{
  "compilerOptions": {
    "composite": true,
    "lib": ["es5"],
    "module": "esnext",
    "moduleResolution": "bundler",
    "paths": {
      "@/*": ["./*"]
    }
  }
}
// @Filename: /home/src/workspaces/project/packages/app/lib/index.ts
Tooltip/**/
// @link: /home/src/workspaces/project/packages/common -> /home/src/workspaces/project/node_modules/@company/common"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_module_specifiers(t, "", &vec!["@company/common".to_string()], None);
    done();
}
