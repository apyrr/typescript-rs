#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_cross_project_base_url_to_dist() {
    let mut t = TestingT;
    run_test_auto_import_cross_project_base_url_to_dist(&mut t);
}

fn run_test_auto_import_cross_project_base_url_to_dist(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportCrossProject_baseUrl_toDist") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/common/tsconfig.json
{
  "compilerOptions": {
    "lib": ["es5"],
    "module": "commonjs",
    "outDir": "dist",
    "composite": true
  },
  "include": ["src"]
}
// @Filename: /home/src/workspaces/project/common/src/MyModule.ts
export function square(n: number) {
  return n * 2;
}
// @Filename: /home/src/workspaces/project/web/tsconfig.json
{
  "compilerOptions": {
    "lib": ["es5"],
    "module": "esnext",
    "moduleResolution": "node",
    "noEmit": true,
    "baseUrl": "."
  },
  "include": ["src"],
  "references": [{ "path": "../common" }]
}
// @Filename: /home/src/workspaces/project/web/src/MyApp.ts
import { square } from "../../common/dist/src/MyModule";
// @Filename: /home/src/workspaces/project/web/src/Helper.ts
export function saveMe() {
  square/**/(2);
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/home/src/workspaces/project/web/src/Helper.ts");
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["../../common/src/MyModule".to_string()],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
            ..Default::default()
        }),
    );
    done();
}
