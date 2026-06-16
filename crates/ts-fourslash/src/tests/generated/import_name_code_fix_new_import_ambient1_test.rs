#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_ambient1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_ambient1(&mut t);
}

fn run_test_import_name_code_fix_new_import_ambient1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import d from "other-ambient-module";
import * as ns from "yet-another-ambient-module";
var x = v1/*0*/ + 5;
// @Filename: ambientModule.ts
declare module "ambient-module" {
   export function f1();
   export var v1;
}
// @Filename: otherAmbientModule.ts
declare module "other-ambient-module" {
   export default function f2();
}
// @Filename: yetAnotherAmbientModule.ts
declare module "yet-another-ambient-module" {
   export function f3();
   export var v3;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { v1 } from "ambient-module";
import d from "other-ambient-module";
import * as ns from "yet-another-ambient-module";
var x = v1 + 5;"#
                .to_string(),
        ],
        None,
    );
    done();
}
