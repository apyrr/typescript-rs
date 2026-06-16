#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_tagged_templates_nested2() {
    let mut t = TestingT;
    run_test_signature_help_tagged_templates_nested2(&mut t);
}

fn run_test_signature_help_tagged_templates_nested2(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpTaggedTemplatesNested2") {
        return;
    }
    let content = r#"function f(templateStrings, x, y, z) { return 10; }
function g(templateStrings, x, y, z) { return ""; }

f `/*1*/a $/*2*/{ /*3*/g /*4*/`alpha ${ 123 } beta ${ 456 } gamma`/*5*/ }/*6*/ b $/*7*/{ /*8*/g /*9*/`txt`/*10*/ } /*11*/c ${ /*12*/g /*13*/`aleph ${ 123 } beit`/*14*/ } d/*15*/`;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.go_to_marker(t, &marker);
        f.verify_signature_help_options(
            t,
            VerifySignatureHelpOptions {
                text: Some("f(templateStrings: any, x: any, y: any, z: any): number".to_string()),
                parameter_name: None,
                parameter_span: None,
                parameter_count: Some(4),
                overloads_count: 0,
            },
        );
    }
    done();
}
