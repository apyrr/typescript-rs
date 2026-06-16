#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_cross_file_quick_info_exported_type_does_not_use_import_type() {
    let mut t = TestingT;
    run_test_cross_file_quick_info_exported_type_does_not_use_import_type(&mut t);
}

fn run_test_cross_file_quick_info_exported_type_does_not_use_import_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: b.ts
export interface B {}
export function foob(): {
    x: B,
    y: B
} {
    return null as any;
}
// @Filename: a.ts
import { foob } from "./b";
const thing/*1*/ = foob(/*2*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "const thing: {\n    x: B;\n    y: B;\n}", "");
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foob(): { x: B; y: B; }".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
