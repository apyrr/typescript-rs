#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_file1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_file1(&mut t);
}

fn run_test_import_name_code_fix_new_import_file1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|/// <reference path="./tripleSlashReference.ts" />
f1/*0*/();|]
// @Filename: Module.ts
export function f1() {}
export var v1 = 5;
// @Filename: tripleSlashReference.ts
var x = 5;/*dummy*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"/// <reference path="./tripleSlashReference.ts" />

import { f1 } from "./Module";

f1();"#
                .to_string(),
        ],
        None,
    );
    done();
}
