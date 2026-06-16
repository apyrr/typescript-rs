#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_type_import5() {
    let mut t = TestingT;
    run_test_auto_import_type_import5(&mut t);
}

fn run_test_auto_import_type_import5(t: &mut TestingT) {
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
export type y = 8
export const Y = 9;
export const Z = 10;
// @Filename: /exports2.ts
export const d = 0;
export const D = 1;
export const e = 2;
export const E = 3;
// @Filename: /index0.ts
import { type X, type Y, type Z } from "./exports1";
const foo: x/*0*/;
const bar: y;
// @Filename: /index1.ts
import { A, B, type X, type Y, type Z } from "./exports1";
const foo: x/*1*/;
const bar: y;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "0");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::First,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::First,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "1");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, B, type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { A, B, type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, B, type x, type X, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
            r#"import { A, B, type X, type y, type Y, type Z } from "./exports1";
const foo: x;
const bar: y;"#
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    done();
}
