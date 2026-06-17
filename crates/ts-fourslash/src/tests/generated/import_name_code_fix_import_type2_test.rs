#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_import_type2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_import_type2(&mut t);
}

fn run_test_import_name_code_fix_import_type2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_importType2") {
        return;
    }
    let content = r#"// @verbatimModuleSyntax: true
// @module: es2015
// @Filename: /exports1.ts
export default interface SomeType {}
export interface OtherType {}
export interface OtherOtherType {}
export const someValue = 0;
// @Filename: /a.ts
import type SomeType from "./exports1.js";
someValue/*a*/
// @Filename: /b.ts
import { someValue } from "./exports1.js";
const b: SomeType/*b*/ = someValue;
// @Filename: /c.ts
import type SomeType from "./exports1.js";
const x: OtherType/*c*/
// @Filename: /d.ts
import type { OtherType } from "./exports1.js";
const x: OtherOtherType/*d*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "a");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import type SomeType from "./exports1.js";
import { someValue } from "./exports1.js";
someValue"#
            .to_string()],
        None,
    );
    f.go_to_marker(t, "b");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import type SomeType from "./exports1.js";
import { someValue } from "./exports1.js";
const b: SomeType = someValue;"#
            .to_string()],
        None,
    );
    f.go_to_marker(t, "c");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import type { OtherType } from "./exports1.js";
import type SomeType from "./exports1.js";
const x: OtherType"#
            .to_string()],
        None,
    );
    f.go_to_marker(t, "d");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import type { OtherOtherType, OtherType } from "./exports1.js";
const x: OtherOtherType"#
                .to_string(),
        ],
        None,
    );
    done();
}
