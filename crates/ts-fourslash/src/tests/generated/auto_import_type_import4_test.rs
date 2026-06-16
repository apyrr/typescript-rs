#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_type_import4() {
    let mut t = TestingT;
    run_test_auto_import_type_import4(&mut t);
}

fn run_test_auto_import_type_import4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @verbatimModuleSyntax: true
// @target: esnext
// @Filename: /exports1.ts
export const a = 0;
export const A = 1;
export const b = 2;
export const B = 3;
export const c = 4;
export const C = 5;
export type x = 6;
export const X = 7;
export const Y = 8;
export const Z = 9;
// @Filename: /exports2.ts
export const d = 0;
export const D = 1;
export const e = 2;
export const E = 3;
// @Filename: /index0.ts
import { A, B, C } from "./exports1";
a/*0*//*0a*/;
b;
// @Filename: /index1.ts
import { A, B, C, type Y, type Z } from "./exports1";
a/*1*//*1a*//*1b*//*1c*/;
b;
// @Filename: /index2.ts
import { A, a, B, b, type Y, type Z } from "./exports1";
import { E } from "./exports2";
d/*2*//*2a*//*2b*//*2c*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "0");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C } from "./exports1";
a;
b;"#
            .to_string(),
            r#"import { A, b, B, C } from "./exports1";
a;
b;"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "0a");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C } from "./exports1";
a;
b;"#
            .to_string(),
            r#"import { A, b, B, C } from "./exports1";
a;
b;"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "1");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
            r#"import { A, b, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "1a");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
            r#"import { A, b, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "1b");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
            r#"import { A, b, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "1c");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
            r#"import { A, b, B, C, type Y, type Z } from "./exports1";
a;
b;"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "2");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, a, B, b, type Y, type Z } from "./exports1";
import { d, E } from "./exports2";
d"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "2a");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, a, B, b, type Y, type Z } from "./exports1";
import { E, d } from "./exports2";
d"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "2b");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, a, B, b, type Y, type Z } from "./exports1";
import { d, E } from "./exports2";
d"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "2c");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, a, B, b, type Y, type Z } from "./exports1";
import { E, d } from "./exports2";
d"#
            .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    done();
}
