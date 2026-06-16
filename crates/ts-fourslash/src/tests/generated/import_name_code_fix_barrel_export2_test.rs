#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_barrel_export2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_barrel_export2(&mut t);
}

fn run_test_import_name_code_fix_barrel_export2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: commonjs
// @baseUrl: /
// @Filename: /proj/foo/a.ts
export const A = 0;
// @Filename: /proj/foo/b.ts
export {};
A/*sibling*/
// @Filename: /proj/foo/index.ts
export * from "./a";
export * from "./b";
// @Filename: /proj/index.ts
export * from "./foo";
export * from "./src";
// @Filename: /proj/src/a.ts
export {};
A/*parent*/
// @Filename: /proj/src/utils.ts
export function util() { return "util"; }
export { A } from "../foo/a";
// @Filename: /proj/src/index.ts
export * from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "sibling",
        &vec![
            "proj/foo/a".to_string(),
            "proj/src/utils".to_string(),
            "proj".to_string(),
            "proj/foo".to_string(),
        ],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
            ..Default::default()
        }),
    );
    f.verify_import_fix_module_specifiers(
        t,
        "parent",
        &vec![
            "proj/foo".to_string(),
            "proj/foo/a".to_string(),
            "proj/src/utils".to_string(),
            "proj".to_string(),
        ],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
            ..Default::default()
        }),
    );
    done();
}
