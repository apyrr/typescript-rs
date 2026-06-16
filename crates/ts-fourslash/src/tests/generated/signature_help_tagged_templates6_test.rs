#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_tagged_templates6() {
    let mut t = TestingT;
    run_test_signature_help_tagged_templates6(&mut t);
}

fn run_test_signature_help_tagged_templates6(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpTaggedTemplates6") {
        return;
    }
    let content = r#"function f(templateStrings, x, y, z) { return 10; }
function g(templateStrings, x, y, z) { return ""; }

f ` qwerty ${ 123 } asdf ${   41234   }  zxcvb ${ g `/*1*/ /*2*/   /*3*/` }    `"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.go_to_marker(t, &marker);
        f.verify_signature_help_options(
            t,
            VerifySignatureHelpOptions {
                text: Some("g(templateStrings: any, x: any, y: any, z: any): string".to_string()),
                parameter_name: Some("templateStrings".to_string()),
                parameter_span: Some("templateStrings: any".to_string()),
                parameter_count: Some(4),
                overloads_count: 0,
            },
        );
    }
    done();
}
