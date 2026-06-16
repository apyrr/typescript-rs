#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_for_nonlocal_type_does_not_use_import_type() {
    let mut t = TestingT;
    run_test_signature_help_for_nonlocal_type_does_not_use_import_type(&mut t);
}

fn run_test_signature_help_for_nonlocal_type_does_not_use_import_type(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpForNonlocalTypeDoesNotUseImportType") {
        return;
    }
    let content = r#"// @Filename: exporter.ts
export interface Thing {}
export const Foo: () => Thing = null as any;
// @Filename: usage.ts
import {Foo} from "./exporter"
function f(p = Foo()): void {}
f(/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(p?: Thing): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
