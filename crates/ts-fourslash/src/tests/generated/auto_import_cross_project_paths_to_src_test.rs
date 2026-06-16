#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_cross_project_paths_to_src() {
    let mut t = TestingT;
    run_test_auto_import_cross_project_paths_to_src(&mut t);
}

fn run_test_auto_import_cross_project_paths_to_src(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportCrossProject_paths_toSrc") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/packages/app/package.json
{ "name": "app", "dependencies": { "dep": "*" } }
// @Filename: /home/src/workspaces/project/packages/app/tsconfig.json
{
  "compilerOptions": {
    "lib": ["es5"],
    "module": "commonjs",
    "outDir": "dist",
    "rootDir": "src",
    "baseUrl": ".",
    "paths": {
      "dep": ["../dep/src/main"],
      "dep/*": ["../dep/*"]
    }
  }
  "references": [{ "path": "../dep" }]
}
// @Filename: /home/src/workspaces/project/packages/app/src/index.ts
dep1/*1*/;
// @Filename: /home/src/workspaces/project/packages/app/src/utils.ts
dep2/*2*/;
// @Filename: /home/src/workspaces/project/packages/app/src/a.ts
import "dep";
// @Filename: /home/src/workspaces/project/packages/dep/package.json
{ "name": "dep", "main": "dist/main.js", "types": "dist/main.d.ts" }
// @Filename: /home/src/workspaces/project/packages/dep/tsconfig.json
{
  "compilerOptions": { "lib": ["es5"], "outDir": "dist", "rootDir": "src", "module": "commonjs" }
}
// @Filename: /home/src/workspaces/project/packages/dep/src/main.ts
import "./sub/folder";
export const dep1 = 0;
// @Filename: /home/src/workspaces/project/packages/dep/src/sub/folder/index.ts
export const dep2 = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "1");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { dep1 } from "dep";

dep1;"#
                .to_string(),
        ],
        None,
    );
    f.go_to_marker(t, "2");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { dep2 } from "dep/src/sub/folder";

dep2;"#
                .to_string(),
        ],
        None,
    );
    done();
}
