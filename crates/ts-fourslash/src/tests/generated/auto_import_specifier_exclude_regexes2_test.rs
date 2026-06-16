#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_specifier_exclude_regexes2() {
    let mut t = TestingT;
    run_test_auto_import_specifier_exclude_regexes2(&mut t);
}

fn run_test_auto_import_specifier_exclude_regexes2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "module": "preserve",
        "paths": {
            "@app/*": ["./src/*"]
        }
    }
}
// @Filename: /src/utils.ts
export function add(a: number, b: number) {}
// @Filename: /src/index.ts
add/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["./utils".to_string()], None);
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["@app/utils".to_string()],
        Some(UserPreferences {
            auto_import_specifier_exclude_regexes: vec!["^\\./".to_string()],
            ..Default::default()
        }),
    );
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["@app/utils".to_string()],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
            ..Default::default()
        }),
    );
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["./utils".to_string()],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
            auto_import_specifier_exclude_regexes: vec!["^@app/".to_string()],
            ..Default::default()
        }),
    );
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec![],
        Some(UserPreferences {
            auto_import_specifier_exclude_regexes: vec!["utils".to_string()],
            ..Default::default()
        }),
    );
    done();
}
