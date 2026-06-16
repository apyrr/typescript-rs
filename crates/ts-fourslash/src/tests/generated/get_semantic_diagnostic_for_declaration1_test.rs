#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_semantic_diagnostic_for_declaration1() {
    let mut t = TestingT;
    run_test_get_semantic_diagnostic_for_declaration1(&mut t);
}

fn run_test_get_semantic_diagnostic_for_declaration1(t: &mut TestingT) {
    if should_skip_if_failing("TestGetSemanticDiagnosticForDeclaration1") {
        return;
    }
    let content = r"// @declaration: true
// @Filename: File.d.ts
declare var v: string;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    done();
}
