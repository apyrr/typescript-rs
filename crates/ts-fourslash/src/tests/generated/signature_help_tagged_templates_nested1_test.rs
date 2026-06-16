#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_tagged_templates_nested1() {
    let mut t = TestingT;
    run_test_signature_help_tagged_templates_nested1(&mut t);
}

fn run_test_signature_help_tagged_templates_nested1(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpTaggedTemplatesNested1") {
        return;
    }
    let content = r#"function f(templateStrings, x, y, z) { return 10; }
function g(templateStrings, x, y, z) { return ""; }

f `a ${ g `/*1*/alpha/*2*/ ${/*3*/ 12/*4*/3 /*5*/} beta /*6*/${ /*7*/456 /*8*/} gamma/*9*/` } b ${ g `/*10*/txt/*11*/` } c ${ g `/*12*/aleph /*13*/$/*14*/{ 12/*15*/3 } beit/*16*/` } d`;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.go_to_marker(t, &marker);
        f.verify_signature_help_options(
            t,
            VerifySignatureHelpOptions {
                text: Some("g(templateStrings: any, x: any, y: any, z: any): string".to_string()),
                parameter_name: None,
                parameter_span: None,
                parameter_count: Some(4),
                overloads_count: 0,
            },
        );
    }
    done();
}
