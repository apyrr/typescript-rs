#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_package_json_imports_case_sensitivity() {
    let mut t = TestingT;
    run_test_auto_import_package_json_imports_case_sensitivity(&mut t);
}

fn run_test_auto_import_package_json_imports_case_sensitivity(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r##"// @module: node18
// @allowImportingTsExtensions: true
// @Filename: /package.json
{
  "type": "module",
  "imports": {
    "#src/*": "./SRC/*"
  }
}
// @Filename: /src/add.ts
export function add(a: number, b: number) {}
// @Filename: /src/index.ts
add/*imports*/;"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "imports",
        &vec!["#src/add.ts".to_string()],
        Some(UserPreferences {
            import_module_specifier_preference:
                modulespecifiers::ImportModuleSpecifierPreference::NonRelative,
            ..Default::default()
        }),
    );
    done();
}
