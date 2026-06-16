#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_type6() {
    let mut t = TestingT;
    run_test_organize_imports_type6(&mut t);
}

fn run_test_organize_imports_type6(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImportsType6") {
        return;
    }
    let content = r#"import { type a, A, b } from "foo";
interface Use extends A {}
console.log(a, b);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import { type a, A, b } from "foo";
interface Use extends A {}
console.log(a, b);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type a, A, b } from \"foo1\";");
    f.verify_organize_imports(
        t,
        r#"import { type a, A, b } from "foo1";
interface Use extends A {}
console.log(a, b);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSUnknown,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type a, A, b } from \"foo2\";");
    f.verify_organize_imports(
        t,
        r#"import { type a, A, b } from "foo2";
interface Use extends A {}
console.log(a, b);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSTrue,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type a, A, b } from \"foo3\";");
    f.verify_organize_imports(
        t,
        r#"import { A, type a, b } from "foo3";
interface Use extends A {}
console.log(a, b);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    done();
}
