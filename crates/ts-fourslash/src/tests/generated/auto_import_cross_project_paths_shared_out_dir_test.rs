#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_cross_project_paths_shared_out_dir() {
    let mut t = TestingT;
    run_test_auto_import_cross_project_paths_shared_out_dir(&mut t);
}

fn run_test_auto_import_cross_project_paths_shared_out_dir(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportCrossProject_paths_sharedOutDir") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/tsconfig.base.json
{
  "compilerOptions": {
    "lib": ["es5"],
    "module": "commonjs",
    "baseUrl": ".",
    "paths": {
      "packages/*": ["./packages/*"]
    }
  }
}
// @Filename: /home/src/workspaces/project/packages/app/tsconfig.json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": { "outDir": "../../dist/packages/app" },
  "references": [{ "path": "../dep" }]
}
// @Filename: /home/src/workspaces/project/packages/app/index.ts
dep/**/
// @Filename: /home/src/workspaces/project/packages/app/utils.ts
import "packages/dep";
// @Filename: /home/src/workspaces/project/packages/dep/tsconfig.json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": { "outDir": "../../dist/packages/dep" }
}
// @Filename: /home/src/workspaces/project/packages/dep/index.ts
import "./sub/folder";
// @Filename: /home/src/workspaces/project/packages/dep/sub/folder/index.ts
export const dep = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { dep } from "packages/dep/sub/folder";

dep"#
                .to_string(),
        ],
        None,
    );
    done();
}
