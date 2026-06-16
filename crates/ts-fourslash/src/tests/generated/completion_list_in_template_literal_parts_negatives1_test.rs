#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_template_literal_parts_negatives1() {
    let mut t = TestingT;
    run_test_completion_list_in_template_literal_parts_negatives1(&mut t);
}

fn run_test_completion_list_in_template_literal_parts_negatives1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"` + "`" + `/*0*/ /*1*/$ /*2*/{ /*3*/$/*4*/{ 10 + 1.1 }/*5*/ 12312/*6*/` + "`" + `

` + "`" + `asdasd$/*7*/{ 2 + 1.1 }/*8*/ 12312 /*9*/{/*10*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Markers(f.markers()), None);
    done();
}
