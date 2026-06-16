#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_import_type3() {
    let mut t = TestingT;
    run_test_import_name_code_fix_import_type3(&mut t);
}

fn run_test_import_name_code_fix_import_type3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @verbatimModuleSyntax: true
// @module: es2015
// @Filename: /exports.ts
class SomeClass {}
export type { SomeClass };
// @Filename: /a.ts
import {} from "./exports.js";
function takeSomeClass(c: SomeClass/**/)"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { type SomeClass } from "./exports.js";
function takeSomeClass(c: SomeClass)"#
                .to_string(),
        ],
        None,
    );
    done();
}
