#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_type1() {
    let mut t = TestingT;
    run_test_organize_imports_type1(&mut t);
}

fn run_test_organize_imports_type1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowSyntheticDefaultImports: true
// @moduleResolution: bundler
// @noUnusedLocals: true
// @target: es2018
import { A } from "foo";
import { type B } from "foo";
import { C } from "foo";
import { type E } from "foo";
import { D } from "foo";

console.log(A, B, C, D, E);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import { A, C, D, type B, type E } from "foo";

console.log(A, B, C, D, E);"#,
        "source.organizeImports",
        None,
    );
    f.verify_organize_imports(
        t,
        r#"import { A, type B, C, D, type E } from "foo";

console.log(A, B, C, D, E);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r#"import { type B, type E, A, C, D } from "foo";

console.log(A, B, C, D, E);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::First,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r#"import { A, C, D, type B, type E } from "foo";

console.log(A, B, C, D, E);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    done();
}
