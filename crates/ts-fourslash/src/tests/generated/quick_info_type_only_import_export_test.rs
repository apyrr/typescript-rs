#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_type_only_import_export() {
    let mut t = TestingT;
    run_test_quick_info_type_only_import_export(&mut t);
}

fn run_test_quick_info_type_only_import_export(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /a.ts
export type A = number;
export const A = 42;
export type B = number;
export const B = 42;

type C = number;
const C = 42;
export type { C };
type D = number;
const D = 42;
export { type D ];
// @Filename: /b.ts
import type { A/*1*/ } from './a';
import { type B/*2*/ } from './a';
import { C/*3*/, D/*4*/ } from './a';
export type { A/*5*/ } from './a';
export { type B/*6*/ } from './a';
export { C/*7*/, D/*8*/ } from './a';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(alias) type A = number\n(alias) const A: 42\nimport A",
        "",
    );
    f.verify_quick_info_at(
        t,
        "2",
        "(alias) type B = number\n(alias) const B: 42\nimport B",
        "",
    );
    f.verify_quick_info_at(
        t,
        "3",
        "(alias) type C = number\n(alias) const C: 42\nimport C",
        "",
    );
    f.verify_quick_info_at(
        t,
        "4",
        "(alias) type D = number\n(alias) const D: 42\nimport D",
        "",
    );
    f.verify_quick_info_at(
        t,
        "5",
        "(alias) type A = number\n(alias) const A: 42\nexport A",
        "",
    );
    f.verify_quick_info_at(
        t,
        "6",
        "(alias) type B = number\n(alias) const B: 42\nexport B",
        "",
    );
    f.verify_quick_info_at(
        t,
        "7",
        "(alias) type C = number\n(alias) const C: 42\nexport C",
        "",
    );
    f.verify_quick_info_at(
        t,
        "8",
        "(alias) type D = number\n(alias) const D: 42\nexport D",
        "",
    );
    done();
}
