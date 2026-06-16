#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_file_all_comments() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_file_all_comments(&mut t);
}

fn run_test_import_name_code_fix_new_import_file_all_comments(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|/*!
 * This is a license or something
 */
/// <reference types="node" />
/// <reference path="./a.ts" />
/// <amd-dependency path="./b.ts" />
/**
 * This is a comment intended to be attached to this interface
 */
export interface SomeInterface {
}
f1/*0*/();|]
// @Filename: module.ts
export function f1() {}
export var v1 = 5;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"/*!
 * This is a license or something
 */
/// <reference types="node" />
/// <reference path="./a.ts" />
/// <amd-dependency path="./b.ts" />

import { f1 } from "./module";

/**
 * This is a comment intended to be attached to this interface
 */
export interface SomeInterface {
}
f1();"#
                .to_string(),
        ],
        None,
    );
    done();
}
