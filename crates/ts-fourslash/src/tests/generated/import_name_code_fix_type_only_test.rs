#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_type_only() {
    let mut t = TestingT;
    run_test_import_name_code_fix_type_only(&mut t);
}

fn run_test_import_name_code_fix_type_only(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_typeOnly") {
        return;
    }
    let content = r"// @module: esnext
// @verbatimModuleSyntax: true
// @Filename: types.ts
export class A {}
// @Filename: index.ts
const a: /**/A";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import type { A } from "./types";

const a: A"#
                .to_string(),
        ],
        None,
    );
    done();
}
