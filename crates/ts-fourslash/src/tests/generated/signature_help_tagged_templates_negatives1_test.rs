#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_tagged_templates_negatives1() {
    let mut t = TestingT;
    run_test_signature_help_tagged_templates_negatives1(&mut t);
}

fn run_test_signature_help_tagged_templates_negatives1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function f(templateStrings, x, y, z) { return 10; }
function g(templateStrings, x, y, z) { return ""; }

/*1*/f/*2*/ /*3*/` + "`" + ` qwerty ${ 123 } asdf ${   41234   }  zxcvb ${ g ` + "`" + `    ` + "`" + ` }     ` + "`" + `/*4*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &f.marker_names());
    done();
}
