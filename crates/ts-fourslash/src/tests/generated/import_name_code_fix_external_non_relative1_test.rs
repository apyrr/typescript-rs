#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_external_non_relative1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_external_non_relative1(&mut t);
}

fn run_test_import_name_code_fix_external_non_relative1(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_externalNonRelative1") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/tsconfig.base.json
{
  "compilerOptions": {
    "module": "commonjs",
    "lib": ["es5"],
    "paths": {
      "pkg-1/*": ["./packages/pkg-1/src/*"],
      "pkg-2/*": ["./packages/pkg-2/src/*"]
    }
  }
}
// @Filename: /home/src/workspaces/project/packages/pkg-1/package.json
{ "dependencies": { "pkg-2": "*" } }
// @Filename: /home/src/workspaces/project/packages/pkg-1/tsconfig.json
{
  "extends": "../../tsconfig.base.json",
  "references": [
    { "path": "../pkg-2" }
  ]
}
// @Filename: /home/src/workspaces/project/packages/pkg-1/src/index.ts
Pkg2/*external*/
// @Filename: /home/src/workspaces/project/packages/pkg-2/package.json
{ "types": "dist/index.d.ts" }
// @Filename: /home/src/workspaces/project/packages/pkg-2/tsconfig.json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": { "outDir": "dist", "rootDir": "src", "composite": true, "lib": ["es5"] }
}
// @Filename: /home/src/workspaces/project/packages/pkg-2/src/index.ts
import "./utils";
// @Filename: /home/src/workspaces/project/packages/pkg-2/src/utils.ts
export const Pkg2 = {};
// @Filename: /home/src/workspaces/project/packages/pkg-2/src/blah/foo/data.ts
Pkg2/*internal*/
// @link: /home/src/workspaces/project/packages/pkg-2 -> /home/src/workspaces/project/packages/pkg-1/node_modules/pkg-2"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    {
        let mut opts = f.get_options();
        opts.format_code_settings.new_line_character = "\\n".to_string();
        f.configure(t, opts);
    }
    f.go_to_marker(t, "external");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { Pkg2 } from "pkg-2/utils";

Pkg2"#
                .to_string(),
        ],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "internal");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { Pkg2 } from "../../utils";

Pkg2"#
                .to_string(),
        ],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative,
            ..Default::default()
        }),
    );
    done();
}
