#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_type2() {
    let mut t = TestingT;
    run_test_organize_imports_type2(&mut t);
}

fn run_test_organize_imports_type2(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImportsType2") {
        return;
    }
    let content = r#"// @allowSyntheticDefaultImports: true
// @moduleResolution: bundler
// @noUnusedLocals: true
// @target: es2018
type A = string;
type B = string;
const C = "hello";
export { A, type B, C };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"type A = string;
type B = string;
const C = "hello";
export { A, C, type B };
"#,
        "source.organizeImports",
        None,
    );
    f.verify_organize_imports(
        t,
        r#"type A = string;
type B = string;
const C = "hello";
export { A, type B, C };
"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r#"type A = string;
type B = string;
const C = "hello";
export { type B, A, C };
"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::First,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r#"type A = string;
type B = string;
const C = "hello";
export { A, C, type B };
"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    done();
}
