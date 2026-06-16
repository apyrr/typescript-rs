#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_relative_path_to_monorepo_package() {
    let mut t = TestingT;
    run_test_auto_import_relative_path_to_monorepo_package(&mut t);
}

fn run_test_auto_import_relative_path_to_monorepo_package(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /home/src/workspaces/project/tsconfig.json
{
  "compilerOptions": {
    "module": "nodenext",
    "lib": ["es5"]
  }
}
// @Filename: /home/src/workspaces/project/packages/app/dist/index.d.ts
import {} from "utils";
export const app: number;
// @Filename: /home/src/workspaces/project/packages/utils/package.json
{ "name": "utils", "version": "1.0.0", "main": "dist/index.js" }
// @Filename: /home/src/workspaces/project/packages/utils/dist/index.d.ts
export const x: number;
// @link: /home/src/workspaces/project/packages/utils -> /home/src/workspaces/project/packages/app/node_modules/utils
// @Filename: /home/src/workspaces/project/script.ts
import {} from "./packages/app/dist/index.js";
x/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["./packages/utils/dist/index.js".to_string()],
        None,
    );
    done();
}
