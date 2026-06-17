#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_base_url1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_base_url1(&mut t);
}

fn run_test_import_name_code_fix_new_import_base_url1(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportBaseUrl1") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "baseUrl": "./a"
    }
}
// @Filename: /a/b/x.ts
export function f1() { };
// @Filename: /a/b/y.ts
[|f1/*0*/();|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a/b/y.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { f1 } from "./x";

f1();"#
            .to_string()],
        None,
    );
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { f1 } from "b/x";

f1();"#
            .to_string()],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
            ..Default::default()
        }),
    );
    done();
}
