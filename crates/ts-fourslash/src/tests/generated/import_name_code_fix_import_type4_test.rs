#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_import_type4() {
    let mut t = TestingT;
    run_test_import_name_code_fix_import_type4(&mut t);
}

fn run_test_import_name_code_fix_import_type4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @preserveValueImports: true
// @isolatedModules: true
// @module: es2015
// @Filename: /exports.ts
export interface SomeInterface {}
export class SomePig {}
// @Filename: /a.ts
import type { SomeInterface } from "./exports.js";
new SomePig/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { SomePig, type SomeInterface } from "./exports.js";
new SomePig"#
                .to_string(),
        ],
        None,
    );
    done();
}
