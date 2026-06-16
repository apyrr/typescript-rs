#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_specifier_exclude_regexes3() {
    let mut t = TestingT;
    run_test_auto_import_specifier_exclude_regexes3(&mut t);
}

fn run_test_auto_import_specifier_exclude_regexes3(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportSpecifierExcludeRegexes3") {
        return;
    }
    let content = r#"// @module: preserve
// @Filename: /node_modules/pkg/package.json
{
    "name": "pkg",
    "version": "1.0.0",
    "exports": {
        ".": "./index.js",
        "./utils": "./utils.js"
    }
}
// @Filename: /node_modules/pkg/utils.d.ts
export function add(a: number, b: number) {}
// @Filename: /node_modules/pkg/index.d.ts
export * from "./utils";
// @Filename: /src/index.ts
add/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["pkg".to_string(), "pkg/utils".to_string()],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["pkg/utils".to_string()],
        Some(UserPreferences {
            auto_import_specifier_exclude_regexes: vec!["^pkg$".to_string()],
            ..Default::default()
        }),
    );
    done();
}
