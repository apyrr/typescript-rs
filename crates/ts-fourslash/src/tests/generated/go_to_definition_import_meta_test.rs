#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_import_meta() {
    let mut t = TestingT;
    run_test_go_to_definition_import_meta(&mut t);
}

fn run_test_go_to_definition_import_meta(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionImportMeta") {
        return;
    }
    let content = r"// @module: esnext
// @Filename: foo.ts
/// <reference path='./bar.d.ts' />
import.me/*reference*/ta;
//@Filename: bar.d.ts
interface ImportMeta {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["reference".to_string()]);
    f.verify_no_errors();
    done();
}
