#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_new_identifier_variable_declaration() {
    let mut t = TestingT;
    run_test_completion_list_new_identifier_variable_declaration(&mut t);
}

fn run_test_completion_list_new_identifier_variable_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var y : (s:string, list/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("2".to_string()), None);
    done();
}
