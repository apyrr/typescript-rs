#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_import_type1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_import_type1(&mut t);
}

fn run_test_import_name_code_fix_import_type1(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_importType1") {
        return;
    }
    let content = r#"// @verbatimModuleSyntax: true
// @module: es2015
// @Filename: /exports.ts
export default someValue = 0;
export function Component() {}
export interface ComponentProps {}
// @Filename: /a.ts
import { Component } from "./exports.js";
interface MoreProps extends /*a*/ComponentProps {}
// @Filename: /b.ts
import someValue from "./exports.js";
interface MoreProps extends /*b*/ComponentProps {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "a");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { Component, type ComponentProps } from "./exports.js";
interface MoreProps extends ComponentProps {}"#
                .to_string(),
        ],
        None,
    );
    f.go_to_marker(t, "b");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import someValue, { type ComponentProps } from "./exports.js";
interface MoreProps extends ComponentProps {}"#
                .to_string(),
        ],
        None,
    );
    done();
}
