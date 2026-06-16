#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_types_declaration_diagnostics_no_server_error() {
    let mut t = TestingT;
    run_test_import_types_declaration_diagnostics_no_server_error(&mut t);
}

fn run_test_import_types_declaration_diagnostics_no_server_error(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @declaration: true
// @Filename: node_modules/foo/index.d.ts
export function f(): I;
export interface I {
  x: number;
}
// @Filename: a.ts
import { f } from "foo";
export const x = f();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file_number(t, 1);
    f.verify_non_suggestion_diagnostics(&[]);
    done();
}
