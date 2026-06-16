#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_external_non_relateive2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_external_non_relateive2(&mut t);
}

fn run_test_import_name_code_fix_external_non_relateive2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_externalNonRelateive2") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/apps/app1/tsconfig.json
{
  "compilerOptions": {
    "module": "commonjs",
    "lib": ["es5"],
    "paths": {
      "shared/*": ["../../shared/*"]
    }
  },
  "include": ["src", "../../shared"]
}
// @Filename: /home/src/workspaces/project/apps/app1/src/index.ts
shared/*internal2external*/
// @Filename: /home/src/workspaces/project/apps/app1/src/app.ts
utils/*internal2internal*/
// @Filename: /home/src/workspaces/project/apps/app1/src/utils.ts
export const utils = 0;
// @Filename: /home/src/workspaces/project/shared/constants.ts
export const shared = 0;
// @Filename: /home/src/workspaces/project/shared/data.ts
shared/*external2external*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    {
        let mut opts = f.get_options();
        opts.format_code_settings.new_line_character = "\\n".to_string();
        f.configure(t, opts);
    }
    f.go_to_marker(t, "internal2external");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { shared } from "shared/constants";

shared"#
                .to_string(),
        ],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "internal2internal");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { utils } from "./utils";

utils"#
                .to_string(),
        ],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "external2external");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { shared } from "./constants";

shared"#
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
