#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_type8() {
    let mut t = TestingT;
    run_test_organize_imports_type8(&mut t);
}

fn run_test_organize_imports_type8(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import { type A, type a, b, B } from "foo";
console.log(a, b, A, B);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import { type A, type a, b, B } from "foo";
console.log(a, b, A, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSUnknown,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type A, type a, b, B } from \"foo1\";");
    f.verify_organize_imports(
        t,
        r#"import { type A, type a, b, B } from "foo1";
console.log(a, b, A, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSUnknown,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::First,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type A, type a, b, B } from \"foo2\";");
    f.verify_organize_imports(
        t,
        r#"import { b, B, type A, type a } from "foo2";
console.log(a, b, A, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSUnknown,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type A, type a, b, B } from \"foo3\";");
    f.verify_organize_imports(
        t,
        r#"import { type A, type a, b, B } from "foo3";
console.log(a, b, A, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSUnknown,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type A, type a, b, B } from \"foo4\";");
    f.verify_organize_imports(
        t,
        r#"import { type A, type a, b, B } from "foo4";
console.log(a, b, A, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSTrue,
            ..Default::default()
        }),
    );
    f.replace_line(t, 0, "import { type A, type a, b, B } from \"foo5\";");
    f.verify_organize_imports(
        t,
        r#"import { type A, B, type a, b } from "foo5";
console.log(a, b, A, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            ..Default::default()
        }),
    );
    done();
}
