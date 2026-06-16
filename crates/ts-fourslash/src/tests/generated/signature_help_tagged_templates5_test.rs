#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_tagged_templates5() {
    let mut t = TestingT;
    run_test_signature_help_tagged_templates5(&mut t);
}

fn run_test_signature_help_tagged_templates5(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpTaggedTemplates5") {
        return;
    }
    let content = r#"function f(templateStrings, x, y, z) { return 10; }
function g(templateStrings, x, y, z) { return ""; }

f ` qwerty ${ 123 } asdf ${   41234   }  zxcvb ${/*1*/ /*2*/g/*3*/ /*4*/`    `/*5*/ /*6*/}    `"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.go_to_marker(t, &marker);
        f.verify_signature_help_options(
            t,
            VerifySignatureHelpOptions {
                text: Some("f(templateStrings: any, x: any, y: any, z: any): number".to_string()),
                parameter_name: Some("z".to_string()),
                parameter_span: Some("z: any".to_string()),
                parameter_count: Some(4),
                overloads_count: 0,
            },
        );
    }
    done();
}
